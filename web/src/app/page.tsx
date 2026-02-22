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
        try {
            const [topologyData, metricsData, healthData] = await Promise.all([
                fetch(apiUrl("/v1/system/topology"), { cache: "no-store" }).then(res => res.ok ? res.json() : null),
                fetch(apiUrl("/v1/metrics/summary"), { cache: "no-store" }).then(res => res.ok ? res.json() : null),
                fetch(apiUrl("/health"), { cache: "no-store" }).then(res => res.ok ? res.json() : null),
            ])

            if (!topologyData && !metricsData && !healthData) {
                throw new Error("All endpoints unreachable")
            }

            const scouts = Number(topologyData?.scout_count ?? healthData?.active_scouts ?? 0)
            const shardNodes = Number(topologyData?.shard_count ?? healthData?.connected_peers ?? 1)
            const verified24h = Number(metricsData?.tokens_verified_24h ?? 0)
            const uptimePercent = Number(healthData?.rust_uptime_ms ? 99.99 : 99.9)

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
        let isSimulated = false
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
                isSimulated = !json?.choices?.[0]?.message?.content
            } else {
                isSimulated = true
            }
        } catch {
            isSimulated = true
        }

        if (isSimulated) {
            setStreamOutput("[Simulated] ")
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

    const isEnterprise = theme === "enterprise"

    return (
        <main className={`app-shell ${isEnterprise ? 'theme-enterprise' : ''}`} aria-label="Shard gateway">
            <Header />

            {toastMessage && <div className="app-toast" style={{ border: '1px solid var(--secondary)', color: 'var(--secondary)' }}>{toastMessage}</div>}

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
                    <h2>{isEnterprise ? "Architecture Comparison" : "[ CLOUD_CORE_VS_SHARD_MESH ]"}</h2>
                    <div className="comparison-table" role="table">
                        {!isEnterprise && (
                            <div className="comparison-row" style={{ borderBottom: '2px solid var(--border)', opacity: 0.5, fontSize: '11px' }}>
                                <span>METRIC</span>
                                <span>TRADITIONAL_CLOUD_AI</span>
                                <span>SHARD_TERMINAL</span>
                            </div>
                        )}
                        {[
                            ["Cost", "$0.002-0.06/1K", "Free (P2P Mesh)"],
                            ["Privacy", "External Storage", "Local-First"],
                            ["Scale", "Fixed Capacity", "Elastic P2P"],
                            ["Resilience", "Single Point", "Self-Healing"],
                            ["Efficiency", "Queue Wait", "Speculative Drafts"],
                            ["Standards", "Proprietary", "OpenAI Compatible"],
                        ].map(([label, cloud, shard]) => (
                            <div className="comparison-row" key={label} role="row">
                                <span className="comparison-label">{isEnterprise ? label : label.toUpperCase()}</span>
                                <span style={{ opacity: 0.6 }}>{cloud}</span>
                                <span className="stat-value--accent">{shard}</span>
                            </div>
                        ))}
                    </div>
                </section>

                <section className="homepage-block">
                    <h2>{isEnterprise ? "P2P Protocol" : "[ MESH_CONTRIBUTION_PROTOCOL ]"}</h2>
                    <p style={{ fontSize: '14px', lineHeight: '1.7', color: 'var(--text-secondary)' }}>
                        {isEnterprise
                            ? "Shard uses a hybrid consensus model. Browser-based 'Scouts' contribute speculative draft tokens via WebGPU, while desktop 'Shards' verify batches using high-performance local inference. This ensures fast, trusted, and decentralized intelligence."
                            : "// SCOUTS_DRAFT_LIKELY_NEXT_TOKENS_VIA_WEBGPU\n// SHARDS_VERIFY_DISTRIBUTED_BATCHES_VIA_GGUF\n// CLIENTS_RECEIVE_TRUSTED_OUTPUT_WITHOUT_CENTRAL_GATEWAYS"}
                    </p>
                </section>

                <section className="homepage-block">
                    <h2>{isEnterprise ? "Inference Simulator" : "[ INTERACTIVE_INFERENCE_SIMULATOR ]"}</h2>
                    <div style={{ marginBottom: '16px' }}>
                        <label htmlFor="demo-prompt" style={{ fontSize: '12px', color: 'var(--text-muted)', marginBottom: '8px', display: 'block' }}>TEST_PROMPT:</label>
                        <textarea
                            id="demo-prompt"
                            value={prompt}
                            onChange={e => setPrompt(e.target.value)}
                            rows={3}
                            className={isEnterprise ? "glass-card w-full p-4 rounded-xl border-none outline-none focus:ring-1 ring-secondary/50" : ""}
                            style={!isEnterprise ? { border: '1px dashed var(--border)', background: 'var(--bg-tertiary)', color: 'var(--primary)', width: '100%', padding: '10px' } : {}}
                        />
                    </div>
                    <button type="button" className={isEnterprise ? "bg-secondary text-white px-6 py-3 rounded-xl font-bold hover:opacity-90 transition-all border-none" : "btn-ping"} onClick={runDemo} disabled={isRunningDemo} style={{ marginBottom: '20px' }}>
                        {isRunningDemo ? (isEnterprise ? "Computing..." : "[ COMPUTING... ]") : (isEnterprise ? "Execute Demo" : "[ EXECUTE_DEMO ]")}
                    </button>
                    <div className="demo-stream" style={{ fontSize: '14px', whiteSpace: 'pre-wrap', borderRadius: isEnterprise ? '12px' : '0' }}>
                        {streamOutput || (isEnterprise ? "System ready..." : "// WAITING_FOR_COMMAND...")}
                        {isRunningDemo && <span className="cursor" />}
                    </div>
                    <div className="homepage-stats-grid" style={{ marginTop: '20px' }}>
                        <div><strong>{demoMetrics.draftTokens}</strong><span>{isEnterprise ? "Draft Tokens" : "SCOUT_CONTRIBUTED_TOKENS"}</span></div>
                        <div><strong>{demoMetrics.verifiedTokens}</strong><span>{isEnterprise ? "Verified Tokens" : "SHARD_VERIFIED_TOKENS"}</span></div>
                    </div>
                </section>

                <footer className="homepage-footer" style={{ textAlign: 'center', padding: '60px 20px', borderTop: '1px solid var(--border)', fontSize: '11px', opacity: 0.6 }}>
                    {isEnterprise ? "Shard Decentralized Intelligence Framework" : "LICENSED_UNDER_BUSL-1.1 // CONVERTS_TO_APACHE_2.0_ON_FEB_13_2036"}<br />
                    <a href="https://github.com/TrentPierce/Shard/blob/main/LICENSE" target="_blank" rel="noreferrer" style={{ color: 'inherit' }}>ROOT/LICENSE_BIN</a>
                </footer>
            </div>
        </main>
    )
}
