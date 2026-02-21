"use client"

import { useEffect, useState } from "react"
import type { NodeMode } from "@/lib/context"
import type { Topology } from "@/lib/swarm"
import { heartbeatShard } from "@/lib/swarm"
import type { ModelProgress } from "@/lib/webllm"
import { apiUrl } from "@/lib/config"
import { modeLabels } from "./Header"
import { listen } from "@tauri-apps/api/event"

interface NetworkStatusProps {
    mode: NodeMode
    topology: Topology | null
    rustStatus: "connected" | "unreachable" | "downloading"
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
        const unlistenProgress = listen<number>("download-progress", (event) => {
            setDownloadProgress(event.payload)
        })

        const unlistenComplete = listen<boolean>("download-complete", () => {
            setDownloadProgress(null)
        })

        return () => {
            unlistenProgress.then((f) => f())
            unlistenComplete.then((f) => f())
        }
    }, [])

    useEffect(() => {
        const fetchPeers = async () => {
            try {
                const res = await fetch(apiUrl("/v1/system/peers"))
                if (res.ok) {
                    const data = await res.json()
                    setPeers(data.peers ?? [])
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
            <section className="sidebar__section">
                <div className="sidebar__section-title">
                    <span className="sidebar__section-icon">⚙️</span>
                    Neural Core
                </div>
                <div className="stat-row">
                    <span className="stat-label">Sync Protocol</span>
                    <span className={`stat-value ${rustStatus === "connected" ? "stat-value--accent" : rustStatus === "downloading" ? "stat-value--warn" : "stat-value--error"}`}>
                        {rustStatus}
                    </span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">Mode</span>
                    <span className="stat-value">{modeLabels[mode]}</span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">Control Port</span>
                    <span className="stat-value">9091 HEX</span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">Net Mesh</span>
                    <span className="stat-value">4001 P2P</span>
                </div>
                {rustStatus === "unreachable" && downloadProgress === null && (
                    <div className="stat-row">
                        <button className="btn-ping" type="button" onClick={() => window.location.reload()}>
                            Re-establish link
                        </button>
                    </div>
                )}
                {downloadProgress !== null && (
                    <div className="stat-row">
                        <span className="stat-label">Weight Transfer</span>
                        <span className="stat-value">{downloadProgress.toFixed(1)}%</span>
                    </div>
                )}
            </section>

            <section className="sidebar__section">
                <div className="sidebar__section-title">
                    <span className="sidebar__section-icon">🪙</span>
                    Ledger
                </div>
                <div className="stat-row">
                    <span className="stat-label">Accumulated Rewards</span>
                    <span className="stat-value stat-value--accent">2,450.00 SHRD</span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">Verifications</span>
                    <span className="stat-value">14.2k</span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">Yield Rate</span>
                    <span className="stat-value">~12/hr</span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">Active Identity</span>
                    <span className="stat-value">0x7F4B…3B9A</span>
                </div>
            </section>

            <section className="sidebar__section">
                <div className="sidebar__section-title">
                    <span className="sidebar__section-icon">🌐</span>
                    Active Swarm ({peers.length})
                </div>
                {peers.length === 0 ? (
                    <div className="stat-label">No remote peers detected. The matrix is currently silent.</div>
                ) : (
                    <ul className="peer-list">
                        {peers.map((p) => (
                            <li key={p.peer_id} className="peer-item">
                                <span className="status-dot status-dot--live" />
                                <span className="peer-item__id">{p.peer_id}</span>
                            </li>
                        ))}
                    </ul>
                )}
                <div className="stat-row" style={{ marginTop: 8 }}>
                    <button className="btn-ping" type="button" disabled={pinging} onClick={doPing}>
                        {pinging ? "Pinging…" : "Ping shard"}
                    </button>
                </div>
                <div className="stat-row">
                    <span className="stat-label">Heartbeat</span>
                    <span className="stat-value">{heartbeat}</span>
                </div>
            </section>

            {(mode === "scout" || mode === "scout-initializing" || webLLMError) && (
                <section className="sidebar__section">
                    <div className="sidebar__section-title">
                        <span className="sidebar__section-icon">🧠</span>
                        Intel Layer
                    </div>
                    {webLLMError ? (
                        <div className="stat-value stat-value--error">{webLLMError}</div>
                    ) : webLLMProgress ? (
                        <>
                            <div className="stat-row">
                                <span className="stat-label">Scout Initializing</span>
                                <span className="stat-value">{Math.round(webLLMProgress.progress * 100)}%</span>
                            </div>
                            <div className="stat-label">{webLLMProgress.text}</div>
                        </>
                    ) : (
                        <div className="stat-label">Accelerating the network with WebGPU draft generation.</div>
                    )}
                </section>
            )}
        </aside>
    )
}
