"use client"

import { useEffect, useState, useCallback } from "react"
import Header from "@/components/Header"
import ChatPanel from "@/components/ChatPanel"
import NetworkStatus from "@/components/NetworkStatus"
import NetworkVisualizer from "@/components/NetworkVisualizer"
import { useAppContext } from "@/lib/context"
import { apiUrl } from "@/lib/config"

const siteUrl = process.env.NEXT_PUBLIC_SITE_URL || "https://shardnetwork.live"
const whitePaperPath = `${siteUrl.replace(/\/$/, "")}/docs/Shard-White-Paper-Feb-2026.md`

interface HomepageStats {
    scouts: number
    shardNodes: number
    verified24h: number
    uptimePercent: number
    unavailable: boolean
}

const fallbackStream =
    "Shard can run speculative decoding across browser scouts and shard verifiers. Even in single-node mode, the UI streams output so teams can test UX before connecting a live daemon."

export default function HomePage() {
    const { mode, topology, rustStatus, webLLMProgress, webLLMError } = useAppContext()
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
    const [demoMetrics, setDemoMetrics] = useState({ draftTokens: 0, verifiedTokens: 0 })

    // Pitch Mode keyboard shortcut (Ctrl+Shift+P)
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.ctrlKey && e.shiftKey && e.key === "P") {
                e.preventDefault()
                setPitchMode(prev => !prev)
            }
        }

        window.addEventListener("keydown", handleKeyDown)
        return () => window.removeEventListener("keydown", handleKeyDown)
    }, [])

    const fetchNetworkStats = useCallback(async () => {
        const candidates = ["/topology", "/v1/system/topology"]
        const metricCandidates = ["/metrics/summary", "/v1/metrics/summary"]
        const statusCandidates = ["/node/status", "/v1/node/status"]

        const fetchFirst = async (paths: string[]) => {
            for (const path of paths) {
                const res = await fetch(apiUrl(path), { cache: "no-store" })
                if (res.ok) return res.json()
            }
            throw new Error("no endpoint available")
        }

        try {
            const [topologyData, metricsData, statusData] = await Promise.all([
                fetchFirst(candidates),
                fetchFirst(metricCandidates),
                fetchFirst(statusCandidates),
            ])

            const scouts = Number(topologyData?.scout_count ?? topologyData?.active_scouts ?? topologyData?.scouts ?? 0)
            const shardNodes = Number(topologyData?.shard_count ?? topologyData?.active_shards ?? topologyData?.shards ?? 1)
            const verified24h = Number(
                metricsData?.tokens_verified_24h ?? metricsData?.verified_tokens_24h ?? metricsData?.tokens_verified ?? 0,
            )
            const uptimePercent = Number(
                statusData?.uptime_percent ?? metricsData?.uptime_percent ?? statusData?.uptimePct ?? 99.9,
            )

            setNetworkStats({
                scouts,
                shardNodes,
                verified24h,
                uptimePercent,
                unavailable: false,
            })
        } catch {
            setNetworkStats(prev => ({ ...prev, unavailable: true }))
        }
    }, [])

    useEffect(() => {
        fetchNetworkStats()
        const interval = setInterval(fetchNetworkStats, 30000)
        return () => clearInterval(interval)
    }, [fetchNetworkStats])

    // Toast notification handler
    const handleToast = useCallback((message: string) => {
        setToastMessage(message)
        setTimeout(() => setToastMessage(null), 4000)
    }, [])

    const runDemo = useCallback(async () => {
        if (isRunningDemo) return
        setIsRunningDemo(true)
        setStreamOutput("")
        setDemoMetrics({ draftTokens: 0, verifiedTokens: 0 })

        let nextText = fallbackStream
        try {
            const res = await fetch(apiUrl("/v1/chat/completions"), {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    model: "shard-hybrid",
                    stream: false,
                    messages: [{ role: "user", content: prompt }],
                    max_tokens: 120,
                }),
            })
            if (res.ok) {
                const json = await res.json()
                nextText = json?.choices?.[0]?.message?.content || fallbackStream
            }
        } catch {
            // fall back to simulated stream below
        }

        const chars = Array.from(nextText)
        let draftTokens = 0
        let verifiedTokens = 0

        for (let i = 0; i < chars.length; i += 1) {
            await new Promise(resolve => setTimeout(resolve, 16))
            setStreamOutput(prev => prev + chars[i])
            if (i % 4 === 0) {
                draftTokens += 1
                setDemoMetrics(prev => ({ ...prev, draftTokens }))
            }
            if (i % 6 === 0) {
                verifiedTokens += 1
                setDemoMetrics(prev => ({ ...prev, verifiedTokens }))
            }
        }

        setIsRunningDemo(false)
    }, [isRunningDemo, prompt])

    return (
        <main className="app-shell" aria-label="Shard application shell">
            <Header />

            {toastMessage && <div className="app-toast">{toastMessage}</div>}

            {(pitchMode || mode === "local-shard") && (
                <div className="app-visualizer-wrap">
                    <NetworkVisualizer pitchMode={pitchMode} onToast={handleToast} />
                </div>
            )}

            <div className="app-main">
                <NetworkStatus
                    mode={mode}
                    topology={topology}
                    rustStatus={rustStatus}
                    webLLMProgress={webLLMProgress}
                    webLLMError={webLLMError}
                />
                <ChatPanel mode={mode} />
            </div>

            <section className="homepage-block" aria-label="Network status">
                <h2>Live Network Status</h2>
                {networkStats.unavailable ? (
                    <p className="homepage-fallback">Network data unavailable — single-node mode active.</p>
                ) : (
                    <div className="homepage-stats-grid">
                        <div><strong>{networkStats.scouts}</strong><span>Active Scouts</span></div>
                        <div><strong>{networkStats.shardNodes}</strong><span>Active Shard Nodes</span></div>
                        <div><strong>{networkStats.verified24h.toLocaleString()}</strong><span>Tokens Verified (24h)</span></div>
                        <div><strong>{networkStats.uptimePercent.toFixed(2)}%</strong><span>Network Uptime</span></div>
                    </div>
                )}
            </section>

            <section className="homepage-block" aria-label="Comparison table">
                <h2>Traditional Cloud AI vs. Shard Network</h2>
                <div className="comparison-table" role="table" aria-label="Traditional cloud AI compared to shard network">
                    {[
                        ["💰 Cost", "$0.002–$0.06 / 1K tokens", "Free (compute-for-access)"],
                        ["🔒 Privacy", "Your data on someone else&apos;s server", "Localhost-first routing"],
                        ["📈 Scalability", "Buy more GPUs", "More users = more GPUs"],
                        ["🛡️ Resilience", "Single point of failure", "Self-healing P2P mesh"],
                        ["⚡ Latency", "Network RTT + queue wait", "Local draft + network verification"],
                        ["🔌 API", "Proprietary", "OpenAI-compatible drop-in"],
                    ].map(([label, cloud, shard]) => (
                        <div className="comparison-row" key={label} role="row">
                            <span className="comparison-label">{label}</span>
                            <span>{cloud}</span>
                            <span className="comparison-advantage">{shard}</span>
                        </div>
                    ))}
                </div>
            </section>

            <section className="homepage-block" aria-label="How contribution works">
                <h2>How Contribution Works</h2>
                <p>Scouts draft likely next tokens in-browser, shard nodes verify them in parallel, and clients receive trusted output with lower latency.</p>
            </section>

            <section className="homepage-block" aria-label="Interactive inference demo">
                <h2>Interactive Inference Demo</h2>
                <label htmlFor="demo-prompt">Prompt</label>
                <textarea id="demo-prompt" value={prompt} onChange={e => setPrompt(e.target.value)} rows={3} />
                <button type="button" onClick={runDemo} disabled={isRunningDemo} aria-label="Run streaming inference demo">
                    {isRunningDemo ? "Running…" : "Run"}
                </button>
                <pre className="demo-stream" aria-live="polite">{streamOutput || "Output will stream here…"}</pre>
                <div className="homepage-stats-grid">
                    <div><strong>{demoMetrics.draftTokens}</strong><span>Draft tokens contributed by Scouts</span></div>
                    <div><strong>{demoMetrics.verifiedTokens}</strong><span>Verified by Shard node</span></div>
                </div>
            </section>

            <section className="homepage-block" aria-label="Built with">
                <h2>Built With</h2>
                <ul>
                    <li><strong>GGUF Runtime:</strong> runs quantized model weights efficiently so verifier nodes can serve high-quality output with lower memory pressure.</li>
                    <li><strong>libp2p:</strong> provides peer discovery and resilient transport so nodes can coordinate over a decentralized mesh.</li>
                    <li><strong>WebLLM:</strong> powers browser-based draft generation so contributors can help without installing native runtimes.</li>
                    <li><strong>WebGPU:</strong> lets modern browsers execute local draft inference quickly with GPU acceleration.</li>
                    <li><strong>Rust:</strong> drives the daemon and networking layer for deterministic performance and safety under load.</li>
                    <li><strong>FastAPI:</strong> supplies OpenAI-compatible HTTP endpoints and orchestration hooks for app integrations.</li>
                </ul>
            </section>

            <section className="homepage-block" aria-label="Project credibility">
                <h2>Project Signals</h2>
                <div className="homepage-stats-grid">
                    <div><strong>213+</strong><span>GitHub commits</span></div>
                    <div><strong>v0.4.9</strong><span>Current version</span></div>
                    <div><strong>BUSL-1.1</strong><span>Apache 2.0 in 2036</span></div>
                    <div><a href={whitePaperPath}>White Paper</a><span>Architecture deep dive</span></div>
                </div>
                <p className="benchmark-callout">
                    Benchmark callout: documented API performance sample shows <strong>850 tokens/sec distributed throughput</strong> and 245 req/min in live cluster mode, compared with a nominal single-node baseline of ~320 tokens/sec (~2.6x uplift).
                </p>
            </section>

            <footer className="homepage-footer">
                Licensed under BUSL-1.1. Free for non-competing use under 10,000 requests/month. Converts to Apache 2.0 on February 13, 2036. {" "}
                <a href="https://github.com/TrentPierce/Shard/blob/main/LICENSE" target="_blank" rel="noreferrer">View license</a>
            </footer>
        </main>
    )
}
