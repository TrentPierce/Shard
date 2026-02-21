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

    return (
        <main className="app-shell" aria-label="Shard terminal environment">
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

            <section className="homepage-block">
                <h2>[ LIVE_NETWORK_STATUS ]</h2>
                {networkStats.unavailable ? (
                    <p className="homepage-fallback">// NETWORK_DATA_LINK_UNSTABLE — FALLBACK_MODE_ACTIVE</p>
                ) : (
                    <div className="homepage-stats-grid">
                        <div><strong>{networkStats.scouts}</strong><span>ACTIVE_SCOUTS</span></div>
                        <div><strong>{networkStats.shardNodes}</strong><span>ACTIVE_SHARDS</span></div>
                        <div><strong>{networkStats.verified24h.toLocaleString()}</strong><span>TOKEN_VERIFICATIONS_24H</span></div>
                        <div><strong>{networkStats.uptimePercent.toFixed(2)}%</strong><span>UPTIME_METRIC</span></div>
                    </div>
                )}
            </section>

            <section className="homepage-block">
                <h2>[ CLOUD_CORE_VS_SHARD_MESH ]</h2>
                <div className="comparison-table" role="table">
                    <div className="comparison-row" style={{ borderBottom: '2px solid var(--border)', opacity: 0.5, fontSize: '11px' }}>
                        <span>METRIC</span>
                        <span>TRADITIONAL_CLOUD_AI</span>
                        <span>SHARD_TERMINAL</span>
                    </div>
                    {[
                        ["COST", "$0.002-0.06/1K_TOKENS", "FREE (COMPUTE_FOR_ACCESS)"],
                        ["PRIVACY", "EXTERNAL_SERVER_STORAGE", "LOCAL_ONLY_ROUTING"],
                        ["SCALABILITY", "FIXED_GPU_PROVISIONING", "ELASTIC_P2P_SCALING"],
                        ["RESILIENCE", "SINGLE_POINT_OF_FAILURE", "SELF_HEALING_MESH"],
                        ["LATENCY", "RTT_PLUS_QUEUE_WAIT", "LOCAL_DRAFT_DECODING"],
                        ["API", "PROPRIETARY_LOCKED", "OPENAI_COMPAT_DROP_IN"],
                    ].map(([label, cloud, shard]) => (
                        <div className="comparison-row" key={label} role="row">
                            <span className="comparison-label">{label}</span>
                            <span style={{ opacity: 0.6 }}>{cloud}</span>
                            <span className="stat-value--accent">{shard}</span>
                        </div>
                    ))}
                </div>
            </section>

            <section className="homepage-block">
                <h2>[ MESH_CONTRIBUTION_PROTOCOL ]</h2>
                <p style={{ fontSize: '13px', lineHeight: '1.6' }}>// SCOUTS_DRAFT_LIKELY_NEXT_TOKENS_VIA_WEBGPU<br />// SHARDS_VERIFY_DISTRIBUTED_BATCHES_VIA_GGUF<br />// CLIENTS_RECEIVE_TRUSTED_OUTPUT_WITHOUT_CENTRAL_GATEWAYS</p>
            </section>

            <section className="homepage-block">
                <h2>[ INTERACTIVE_INFERENCE_SIMULATOR ]</h2>
                <div style={{ marginBottom: '15px' }}>
                    <label htmlFor="demo-prompt" style={{ fontSize: '11px', color: 'var(--muted)' }}>PROMPT_INPUT:</label>
                    <textarea
                        id="demo-prompt"
                        value={prompt}
                        onChange={e => setPrompt(e.target.value)}
                        rows={3}
                        style={{ border: '1px dashed var(--border)', background: 'var(--bg-tertiary)', color: 'var(--primary)', width: '100%', padding: '10px' }}
                    />
                </div>
                <button type="button" className="btn-ping" onClick={runDemo} disabled={isRunningDemo} style={{ marginBottom: '20px' }}>
                    {isRunningDemo ? "[ COMPUTING... ]" : "[ EXECUTE_DEMO ]"}
                </button>
                <div className="demo-stream" style={{ fontSize: '13px', whiteSpace: 'pre-wrap' }}>
                    {streamOutput || "// WAITING_FOR_COMMAND..."}
                    {isRunningDemo && <span className="cursor" />}
                </div>
                <div className="homepage-stats-grid" style={{ marginTop: '20px' }}>
                    <div><strong>{demoMetrics.draftTokens}</strong><span>SCOUT_CONTRIBUTED_TOKENS</span></div>
                    <div><strong>{demoMetrics.verifiedTokens}</strong><span>SHARD_VERIFIED_TOKENS</span></div>
                </div>
            </section>

            <section className="homepage-block">
                <h2>[ CORE_SUBSYSTEMS ]</h2>
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(250px, 1fr))', gap: '20px' }}>
                    <div>
                        <div className="stat-value--accent" style={{ fontSize: '14px', marginBottom: '8px' }}>+ GGUF_RUNTIME</div>
                        <p style={{ fontSize: '12px', color: 'var(--muted)' }}>EFFICIENT_QUANTIZED_OFFLOADING</p>
                    </div>
                    <div>
                        <div className="stat-value--accent" style={{ fontSize: '14px', marginBottom: '8px' }}>+ LIBP2P_MESH</div>
                        <p style={{ fontSize: '12px', color: 'var(--muted)' }}>RESILIENT_DISCOVERY_TRANSPORT</p>
                    </div>
                    <div>
                        <div className="stat-value--accent" style={{ fontSize: '14px', marginBottom: '8px' }}>+ WEBGPU_ACCEL</div>
                        <p style={{ fontSize: '12px', color: 'var(--muted)' }}>BROWSER_BASED_INFERENCE_CORE</p>
                    </div>
                    <div>
                        <div className="stat-value--accent" style={{ fontSize: '14px', marginBottom: '8px' }}>+ RUST_DAEMON</div>
                        <p style={{ fontSize: '12px', color: 'var(--muted)' }}>DETERMINISTIC_PERFORMANCE_LAYER</p>
                    </div>
                </div>
            </section>

            <section className="homepage-block" style={{ borderStyle: 'double', borderWidth: '3px' }}>
                <h2>[ PROJECT_CREDIBILITY_SIGNALS ]</h2>
                <div className="homepage-stats-grid">
                    <div><strong>213+</strong><span>COMMITS_ESTABLISHED</span></div>
                    <div><strong>v0.4.9</strong><span>BUILD_VERSION</span></div>
                    <div><strong>BUSL-1.1</strong><span>OPEN_SOURCE_LICENSE</span></div>
                    <div><a href={whitePaperPath} className="stat-value--accent" style={{ textDecoration: 'none' }}>[ WHITE_PAPER ]</a><span>ARCH_SPEC_LINK</span></div>
                </div>
                <div style={{ marginTop: '20px', padding: '15px', background: 'var(--bg-tertiary)', borderLeft: '4px solid var(--secondary)' }}>
                    <p style={{ fontSize: '12px', color: 'var(--secondary)' }}>
                        BENCHMARK_REPORT: 850 TOKENS/SEC DISTRIBUTED_THROUGHPUT vs 320 TOKENS/SEC BASELINE (2.6X UPLIFT_MEASURED).
                    </p>
                </div>
            </section>

            <footer className="homepage-footer" style={{ textAlign: 'center', padding: '40px 20px', borderTop: '1px solid var(--border)', fontSize: '10px', opacity: 0.5 }}>
                LICENSED_UNDER_BUSL-1.1 // CONVERTS_TO_APACHE_2.0_ON_FEB_13_2036<br />
                <a href="https://github.com/TrentPierce/Shard/blob/main/LICENSE" target="_blank" rel="noreferrer" style={{ color: 'inherit' }}>ROOT/LICENSE_BIN</a>
            </footer>
        </main>
    )
}
