"use client"

import { useEffect, useState } from "react"
import type { NodeMode } from "@/lib/context"
import type { Topology } from "@/lib/swarm"
import { heartbeatShard } from "@/lib/swarm"
import type { ModelProgress } from "@/lib/webllm"
import { apiUrl } from "@/lib/config"
import { modeLabels } from "./Header"
import { Activity, Database, Shield, Zap, RefreshCw } from "lucide-react"

interface NetworkStatusProps {
    mode: NodeMode
    topology: Topology | null
    rustStatus: "connected" | "degraded" | "unreachable" | "downloading"
    webLLMProgress: ModelProgress | null
    webLLMError: string | null
}

interface PeerData {
    peer_id: string
    connected_at: number
    addrs: string[]
}

export default function NetworkStatus({
    mode,
    topology,
    rustStatus,
    webLLMProgress,
    webLLMError,
}: NetworkStatusProps) {
    const [peers, setPeers] = useState<PeerData[]>([])
    const [heartbeat, setHeartbeat] = useState("idle")
    const [pinging, setPinging] = useState(false)
    const [downloadProgress, setDownloadProgress] = useState<number | null>(null)

    useEffect(() => {
        if (typeof window === "undefined" || !(window as any).__TAURI_INTERNALS__) {
            return
        }

        let unlistenProgress: (() => void) | null = null
        let unlistenComplete: (() => void) | null = null
        let disposed = false

        const registerDownloadListeners = async () => {
            const { listen } = await import("@tauri-apps/api/event")

            const progressUnlisten = await listen<number>("download-progress", (event) => {
                setDownloadProgress(event.payload)
            })
            const completeUnlisten = await listen<boolean>("download-complete", () => {
                setDownloadProgress(null)
            })

            if (disposed) {
                progressUnlisten()
                completeUnlisten()
                return
            }

            unlistenProgress = progressUnlisten
            unlistenComplete = completeUnlisten
        }

        registerDownloadListeners().catch((error) => {
            console.warn("Failed to attach Tauri download listeners", error)
        })

        return () => {
            disposed = true
            unlistenProgress?.()
            unlistenComplete?.()
        }
    }, [])

    useEffect(() => {
        const fetchPeers = async () => {
            try {
                const res = await fetch("/api/v1/system/peers")
                if (res.ok) {
                    const data = await res.json()
                    // Handle both {peers: [...]} and {count: N, peers: [...]} formats
                    const peersList = Array.isArray(data.peers) ? data.peers : []
                    setPeers(peersList)
                }
            } catch {
                /* sidecar unreachable */
            }
        }

        fetchPeers()
        const interval = setInterval(fetchPeers, 8000)
        return () => clearInterval(interval)
    }, [])

    const doPing = async () => {
        const addr = topology?.shard_ws_multiaddr ?? topology?.shard_webrtc_multiaddr
        if (!addr) {
            setHeartbeat("no shard address")
            return
        }
        setPinging(true)
        setHeartbeat("dialing…")
        const result = await heartbeatShard(addr)
        if (result.ok) {
            setHeartbeat(`PONG rtt=${result.rttMs?.toFixed(1)}ms`)
        } else {
            setHeartbeat(`failed: ${result.detail}`)
        }
        setPinging(false)
    }

    return (
        <aside className="sidebar" role="complementary">
            <section className="sidebar__section card">
                <div className="sidebar__section-title flex items-center gap-2 mb-4">
                    <Activity size={18} className="text-primary" />
                    Core Node Status
                </div>
                <div className="stat-row">
                    <span className="stat-label">Protocol</span>
                    <span className={`stat-value ${rustStatus === "connected" ? "text-primary" : rustStatus === "downloading" || rustStatus === "degraded" ? "text-accent-amber" : "text-error"}`}>
                        {rustStatus.toUpperCase()}
                    </span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">Subsystem</span>
                    <span className="stat-value">{modeLabels[mode]}</span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">Control Port</span>
                    <span className="stat-value">9091</span>
                </div>
                {rustStatus === "unreachable" && downloadProgress === null && (
                    <button className="btn btn-primary btn-sm w-full mt-4" type="button" onClick={() => window.location.reload()}>
                        Reconnect
                    </button>
                )}
                {downloadProgress !== null && (
                    <div className="mt-4">
                        <div className="stat-label mb-2">Transferring Weights</div>
                        <div className="w-full bg-bg-tertiary rounded-full h-2">
                            <div 
                                className="bg-primary h-2 rounded-full transition-all duration-300" 
                                style={{ width: `${downloadProgress}%` }}
                            />
                        </div>
                        <div className="stat-value text-right mt-1" style={{ fontSize: '12px' }}>{downloadProgress.toFixed(1)}%</div>
                    </div>
                )}
            </section>

            <section className="sidebar__section card mt-4">
                <div className="sidebar__section-title flex items-center gap-2 mb-4">
                    <Database size={18} className="text-secondary" />
                    Allocation
                </div>
                <div className="stat-row">
                    <span className="stat-label">Rewards</span>
                    <span className="stat-value text-primary font-bold">2,450.00 SHRD</span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">Verifications</span>
                    <span className="stat-value">14,288</span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">Yield Avg</span>
                    <span className="stat-value">12.4/hr</span>
                </div>
            </section>

            <section className="sidebar__section card mt-4">
                <div className="sidebar__section-title flex items-center gap-2 mb-4">
                    <Zap size={18} className="text-accent-cyan" />
                    Swarm ({peers.length})
                </div>
                {peers.length === 0 ? (
                    <div className="text-muted italic text-sm mt-2">No remote peers detected</div>
                ) : (
                    <ul className="space-y-2 mt-2">
                        {peers.slice(0, 5).map((p) => (
                            <li key={p.peer_id} className="flex items-center gap-2 text-sm text-secondary">
                                <Shield size={12} />
                                <span className="truncate">{p.peer_id.substring(0, 16)}...</span>
                            </li>
                        ))}
                    </ul>
                )}
                <button 
                    className="btn btn-secondary btn-sm w-full mt-4 flex items-center justify-center gap-2" 
                    type="button" 
                    disabled={pinging} 
                    onClick={doPing}
                >
                    {pinging ? <RefreshCw size={14} className="animate-spin" /> : <Zap size={14} />}
                    {pinging ? "Pinging..." : "Ping Shard"}
                </button>
                <div className="stat-row mt-3">
                    <span className="stat-label">Latency</span>
                    <span className="stat-value" style={{ fontSize: '13px' }}>{heartbeat}</span>
                </div>
            </section>

            {(mode === "scout" || mode === "scout-initializing" || webLLMError) && (
                <section className="sidebar__section card mt-4 border-secondary/30">
                    <div className="sidebar__section-title flex items-center gap-2 mb-4 text-secondary">
                        <Activity size={18} />
                        Intelligence Layer
                    </div>
                    {webLLMError ? (
                        <div className="text-error text-sm">Error: {webLLMError}</div>
                    ) : webLLMProgress ? (
                        <div>
                            <div className="stat-label mb-2">Initializing Scout</div>
                            <div className="w-full bg-bg-tertiary rounded-full h-2">
                                <div 
                                    className="bg-secondary h-2 rounded-full transition-all duration-300" 
                                    style={{ width: `${webLLMProgress.progress * 100}%` }}
                                />
                            </div>
                            <div className="text-muted text-xs mt-2 truncate">{webLLMProgress.text}</div>
                        </div>
                    ) : (
                        <div className="flex items-center gap-2 text-success text-sm font-medium">
                            <Zap size={14} />
                            Draft Generation Active
                        </div>
                    )}
                </section>
            )}

            <style jsx>{`
                .sidebar__section {
                    padding: var(--space-lg);
                }
                .sidebar__section-title {
                    font-weight: 700;
                    font-size: 0.875rem;
                    text-transform: uppercase;
                    letter-spacing: 0.05em;
                    color: var(--text-primary);
                }
                .stat-row {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    margin-bottom: var(--space-sm);
                }
                .stat-label {
                    font-size: 0.8125rem;
                    color: var(--text-secondary);
                }
                .stat-value {
                    font-size: 0.875rem;
                    font-weight: 600;
                    color: var(--text-primary);
                }
            `}</style>
        </aside>
    )
}
