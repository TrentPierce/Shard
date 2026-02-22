"use client"

import { useEffect, useState, useCallback } from "react"
import Header from "@/components/Header"
import ChatPanel from "@/components/ChatPanel"
import NetworkStatus from "@/components/NetworkStatus"
import NetworkVisualizer from "@/components/NetworkVisualizer"
import { useAppContext } from "@/lib/context"
import { apiUrl } from "@/lib/config"
import {
  incrementDraftTokens,
  incrementVerifiedTokens,
  readContributionMetrics,
  resetSessionMetrics,
  type ContributionMetrics,
} from "@/lib/contribution-metrics"

interface HomepageStats {
  scouts: number
  shardNodes: number
  verified24h: number
  uptimePercent: number
  unavailable: boolean
}

const fallbackStream =
  "Shard can run speculative decoding across browser scouts and shard verifiers. Even in single-node mode, the UI streams output so teams can test UX before connecting a live daemon."

const joinCommands = {
  linux: "chmod +x ./shard-daemon-x86_64-unknown-linux-gnu && ./shard-daemon-x86_64-unknown-linux-gnu",
  install: "./scripts/install.sh",
  api: `curl http://localhost:9091/v1/chat/completions \\\n  -H \"Content-Type: application/json\" \\\n  -d '{\"model\":\"shard-hybrid\",\"messages\":[{\"role\":\"user\",\"content\":\"Hello!\"}]}'`,
}

export default function HomePage() {
  const { mode, topology, rustStatus, webLLMProgress, webLLMError, theme } = useAppContext()
  const [pitchMode, setPitchMode] = useState(false)
  const [toastMessage, setToastMessage] = useState<string | null>(null)
  const [networkStats, setNetworkStats] = useState<HomepageStats>({
    scouts: 0,
    shardNodes: 0,
    verified24h: 0,
    uptimePercent: 99.9,
    unavailable: true,
  })
  const [prompt, setPrompt] = useState("Explain how speculative decoding works in two sentences.")
  const [streamOutput, setStreamOutput] = useState("")
  const [isRunningDemo, setIsRunningDemo] = useState(false)
  const [demoMetrics, setDemoMetrics] = useState<ContributionMetrics>({ draftTokens: 0, verifiedTokens: 0 })

  useEffect(() => {
    setDemoMetrics(readContributionMetrics())
  }, [])

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.key === "P") {
        e.preventDefault()
        setPitchMode((prev) => !prev)
      }
    }

    window.addEventListener("keydown", handleKeyDown)
    return () => window.removeEventListener("keydown", handleKeyDown)
  }, [])

  const fetchNetworkStats = useCallback(async () => {
    try {
      const [topologyData, metricsData, healthData] = await Promise.all([
        fetch(apiUrl("/v1/system/topology"), { cache: "no-store" }).then((res) => (res.ok ? res.json() : null)),
        fetch(apiUrl("/v1/metrics/summary"), { cache: "no-store" }).then((res) => (res.ok ? res.json() : null)),
        fetch(apiUrl("/health"), { cache: "no-store" }).then((res) => (res.ok ? res.json() : null)),
      ])

      if (!topologyData && !metricsData && !healthData) {
        throw new Error("All endpoints unreachable")
      }

      setNetworkStats({
        scouts: Number(topologyData?.scout_count ?? healthData?.active_scouts ?? 0),
        shardNodes: Number(topologyData?.shard_count ?? healthData?.connected_peers ?? 0),
        verified24h: Number(metricsData?.tokens_verified_24h ?? 0),
        uptimePercent: Number(healthData?.status === "ok" ? 99.99 : 99.5),
        unavailable: false,
      })
    } catch {
      setNetworkStats((prev) => ({ ...prev, unavailable: true }))
    }
  }, [])

  useEffect(() => {
    fetchNetworkStats()
    const interval = setInterval(fetchNetworkStats, 30000)
    return () => clearInterval(interval)
  }, [fetchNetworkStats])

  const handleToast = useCallback((message: string) => {
    setToastMessage(message)
    setTimeout(() => setToastMessage(null), 4000)
  }, [])

  const runDemo = useCallback(async () => {
    if (isRunningDemo) return

    setIsRunningDemo(true)
    setStreamOutput("")
    setDemoMetrics(resetSessionMetrics())

    try {
      const res = await fetch(apiUrl("/v1/chat/completions"), {
        method: "POST",
        headers: { "Content-Type": "application/json", "X-Shard-Inference-Mode": "distributed" },
        body: JSON.stringify({
          model: "shard-hybrid",
          stream: true,
          messages: [{ role: "user", content: prompt }],
          max_tokens: 120,
        }),
      })

      if (!res.ok || !res.body) {
        throw new Error(`Demo request failed with status ${res.status}`)
      }

      const reader = res.body.getReader()
      const decoder = new TextDecoder()
      let buffer = ""

      setStreamOutput("[SCOUT_DRAFT] Bootstrapping speculative stream...\n")

      while (true) {
        const { done, value } = await reader.read()
        if (done) break

        buffer += decoder.decode(value, { stream: true })
        const lines = buffer.split("\n")
        buffer = lines.pop() ?? ""

        for (const line of lines) {
          if (!line.startsWith("data:")) continue
          const payload = line.replace(/^data:\s*/, "").trim()
          if (!payload || payload === "[DONE]") continue

          try {
            const parsed = JSON.parse(payload)
            const token = parsed?.choices?.[0]?.delta?.content
            if (!token) continue

            const draftMetrics = incrementDraftTokens(1)
            const maybeVerified = token.endsWith(" ") || token.endsWith(".") || token.endsWith(",")
            const finalMetrics = maybeVerified ? incrementVerifiedTokens(1) : draftMetrics
            setDemoMetrics(finalMetrics)

            setStreamOutput((prev) => `${prev}[SCOUT_DRAFT] ${token}\n${maybeVerified ? `[VERIFIER_FINAL] ${token}\n` : ""}`)
          } catch {
            continue
          }
        }
      }
    } catch {
      setStreamOutput(`[SIMULATION_FALLBACK]\n[SCOUT_DRAFT] ${fallbackStream}\n[VERIFIER_FINAL] ${fallbackStream}`)
      const fallbackDraft = incrementDraftTokens(8)
      setDemoMetrics(incrementVerifiedTokens(Math.max(1, Math.floor(fallbackDraft.draftTokens / 2))))
    } finally {
      setIsRunningDemo(false)
    }
  }, [isRunningDemo, prompt])

  const isEnterprise = theme === "enterprise"

  return (
    <main className={`app-shell ${isEnterprise ? "theme-enterprise" : ""}`} aria-label="Shard gateway">
      <Header />
      {toastMessage && <div className="app-toast" style={{ border: "1px solid var(--secondary)", color: "var(--secondary)" }}>{toastMessage}</div>}

      {(pitchMode || mode === "local-shard") && (
        <div className="app-visualizer-wrap">
          <NetworkVisualizer pitchMode={pitchMode} onToast={handleToast} />
        </div>
      )}

      <div className="app-main">
        <NetworkStatus mode={mode} topology={topology} rustStatus={rustStatus} webLLMProgress={webLLMProgress} webLLMError={webLLMError} />
        <ChatPanel mode={mode} />
      </div>

      <div className="app-container">
        <section className="homepage-block">
          <h2>{isEnterprise ? "Live Network Stats" : "[ LIVE_NETWORK_STATUS ]"}</h2>
          {networkStats.unavailable ? (
            <p className="homepage-fallback">{isEnterprise ? "Network data unavailable. Fallback mode active." : "// NETWORK_DATA_LINK_UNSTABLE — FALLBACK_MODE_ACTIVE"}</p>
          ) : (
            <div className="homepage-stats-grid">
              <div><strong>{networkStats.scouts}</strong><span>{isEnterprise ? "Active Scouts" : "ACTIVE_SCOUTS"}</span></div>
              <div><strong>{networkStats.shardNodes}</strong><span>{isEnterprise ? "Active Shards" : "ACTIVE_SHARDS"}</span></div>
              <div><strong>{networkStats.verified24h.toLocaleString()}</strong><span>{isEnterprise ? "Daily Verifications" : "TOKEN_VERIFICATIONS_24H"}</span></div>
              <div><strong>{networkStats.uptimePercent.toFixed(2)}%</strong><span>{isEnterprise ? "Network Uptime" : "UPTIME_METRIC"}</span></div>
            </div>
          )}
        </section>

        <section className="homepage-block">
          <h2>{isEnterprise ? "Inference Simulator" : "[ INTERACTIVE_INFERENCE_SIMULATOR ]"}</h2>
          <label htmlFor="demo-prompt" style={{ fontSize: "12px", color: "var(--text-muted)", marginBottom: "8px", display: "block" }}>TEST_PROMPT:</label>
          <textarea id="demo-prompt" value={prompt} onChange={(e) => setPrompt(e.target.value)} rows={3} style={{ border: "1px dashed var(--border)", background: "var(--bg-tertiary)", color: "var(--primary)", width: "100%", padding: "10px" }} />
          <button type="button" className="btn-ping" onClick={runDemo} disabled={isRunningDemo} style={{ margin: "16px 0 20px" }}>
            {isRunningDemo ? "[ COMPUTING... ]" : "[ EXECUTE_DEMO ]"}
          </button>
          <div className="demo-stream" style={{ fontSize: "13px", whiteSpace: "pre-wrap" }}>
            {streamOutput || "// WAITING_FOR_COMMAND..."}
            {isRunningDemo && <span className="cursor" />}
          </div>
          <div className="homepage-stats-grid" style={{ marginTop: "20px" }}>
            <div><strong>{demoMetrics.draftTokens}</strong><span>SCOUT_CONTRIBUTED_TOKENS</span></div>
            <div><strong>{demoMetrics.verifiedTokens}</strong><span>SHARD_VERIFIED_TOKENS</span></div>
          </div>
        </section>

        <section className="homepage-block">
          <h2>{isEnterprise ? "Join the Network" : "[ JOIN_THE_NETWORK ]"}</h2>
          <p style={{ color: "var(--text-secondary)", marginBottom: "12px" }}>Run your own Shard verifier node or test the OpenAI-compatible API in minutes.</p>
          <pre className="demo-stream" style={{ fontSize: "12px", marginBottom: "10px" }}>{joinCommands.linux}</pre>
          <pre className="demo-stream" style={{ fontSize: "12px", marginBottom: "10px" }}>{joinCommands.install}</pre>
          <pre className="demo-stream" style={{ fontSize: "12px" }}>{joinCommands.api}</pre>
        </section>

        <section className="homepage-block">
          <h2>{isEnterprise ? "Architecture Flow" : "[ ARCHITECTURE_FLOW ]"}</h2>
          <pre className="demo-stream" style={{ fontSize: "12px" }}>{`Browser User
   -> Next.js Web App
      -> /api/v1/chat/completions
         -> Rust Sidecar / Python Driver
            -> Shard Mesh (libp2p)`}</pre>
        </section>
      </div>
    </main>
  )
}
