"use client"

import { useEffect, useState } from "react"
import type { NodeMode } from "@/lib/context"
import type { Topology } from "@/lib/swarm"
import { heartbeatShard } from "@/lib/swarm"
import type { ModelProgress } from "@/lib/webllm"
import { apiUrl } from "@/lib/config"
import { modeLabels } from "./Header"

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
                    [ NEURAL_CORE ]
                </div>
                <div className="stat-row">
                    <span className="stat-label">PROTOCOL</span>
                    <span className={`stat-value ${rustStatus === "connected" ? "stat-value--accent" : rustStatus === "downloading" || rustStatus === "degraded" ? "stat-value--warn" : "stat-value--error"}`}>
                        {rustStatus.toUpperCase()}
                    </span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">SUBSYSTEM</span>
                    <span className="stat-value">{modeLabels[mode].toUpperCase()}</span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">CTL_PORT</span>
                    <span className="stat-value">9091/HEX</span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">MESH_NET</span>
                    <span className="stat-value">4001/P2P</span>
                </div>
                {rustStatus === "unreachable" && downloadProgress === null && (
                    <div className="stat-row" style={{ marginTop: '10px' }}>
                        <button className="btn-ping" type="button" onClick={() => window.location.reload()}>
                            [ RE_ESTABLISH_LINK ]
                        </button>
                    </div>
                )}
                {downloadProgress !== null && (
                    <div className="sidebar__section" style={{ marginTop: '10px', borderStyle: 'dashed' }}>
                        <div className="stat-label">TRANSFERRING_WEIGHTS</div>
                        <div className="stat-value" style={{ fontSize: '10px' }}>
                            [{'#'.repeat(Math.floor(downloadProgress / 5))}{'.'.repeat(20 - Math.floor(downloadProgress / 5))}] {downloadProgress.toFixed(1)}%
                        </div>
                    </div>
                )}
            </section>

            <section className="sidebar__section">
                <div className="sidebar__section-title">
                    [ LEDGER_ALLOCATION ]
                </div>
                <div className="stat-row">
                    <span className="stat-label">REWARDS</span>
                    <span className="stat-value stat-value--accent">2,450.00 SHRD</span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">VERIF_COUNT</span>
                    <span className="stat-value">14,288</span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">YIELD_AVG</span>
                    <span className="stat-value">12.4/HR</span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">ID_SIG</span>
                    <span className="stat-value" style={{ fontSize: '10px' }}>0x7F4B...3B9A</span>
                </div>
            </section>

            <section className="sidebar__section">
                <div className="sidebar__section-title">
                    [ ACTIVE_SWARM ({peers.length}) ]
                </div>
                {peers.length === 0 ? (
                    <div className="stat-label" style={{ fontSize: '11px', fontStyle: 'italic' }}>// NO REMOTE PEERS DETECTED</div>
                ) : (
                    <ul className="peer-list" style={{ borderLeft: '1px solid var(--border)', paddingLeft: '10px' }}>
                        {peers.map((p) => (
                            <li key={p.peer_id} className="peer-item" style={{ padding: '2px 0' }}>
                                <span className="stat-value" style={{ fontSize: '10px' }}>+ {p.peer_id.substring(0, 16)}...</span>
                            </li>
                        ))}
                    </ul>
                )}
                <div className="stat-row" style={{ marginTop: 12 }}>
                    <button className="btn-ping" type="button" disabled={pinging} onClick={doPing}>
                        {pinging ? "[ DIALING... ]" : "[ PING_SHARD ]"}
                    </button>
                </div>
                <div className="stat-row">
                    <span className="stat-label">HEARTBEAT</span>
                    <span className="stat-value" style={{ fontSize: '11px' }}>{heartbeat.toUpperCase()}</span>
                </div>
            </section>

            {(mode === "scout" || mode === "scout-initializing" || webLLMError) && (
                <section className="sidebar__section" style={{ borderColor: 'var(--secondary)' }}>
                    <div className="sidebar__section-title" style={{ color: 'var(--secondary)' }}>
                        [ INTEL_LAYER ]
                    </div>
                    {webLLMError ? (
                        <div className="stat-value--error" style={{ fontSize: '11px' }}>ERR: {webLLMError.toUpperCase()}</div>
                    ) : webLLMProgress ? (
                        <>
                            <div className="stat-label">INITIALIZING_SCOUT</div>
                            <div className="stat-value" style={{ fontSize: '10px', color: 'var(--secondary)' }}>
                                [{'#'.repeat(Math.round(webLLMProgress.progress * 20))}{'.'.repeat(20 - Math.round(webLLMProgress.progress * 20))}] {Math.round(webLLMProgress.progress * 100)}%
                            </div>
                            <div className="stat-label" style={{ fontSize: '10px', marginTop: '4px' }}>{webLLMProgress.text.toUpperCase()}</div>
                        </>
                    ) : (
                        <div className="stat-label" style={{ fontSize: '11px' }}>// WEBGPU_DRAFT_GEN_ACTIVE</div>
                    )}
                </section>
            )}
        </aside>
    )
}
