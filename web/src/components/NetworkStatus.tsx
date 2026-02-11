"use client"

import { useEffect, useState } from "react"
import type { NodeMode } from "@/app/page"
import type { Topology } from "@/lib/swarm"
import { heartbeatOracle } from "@/lib/swarm"

interface NetworkStatusProps {
    mode: NodeMode
    topology: Topology | null
    rustStatus: "connected" | "unreachable"
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
}: NetworkStatusProps) {
    const [peers, setPeers] = useState<PeerData[]>([])
    const [heartbeat, setHeartbeat] = useState("idle")
    const [pinging, setPinging] = useState(false)

    useEffect(() => {
        const fetchPeers = async () => {
            try {
                const res = await fetch("http://127.0.0.1:8000/v1/system/peers")
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
        const addr =
            topology?.oracle_ws_multiaddr ?? topology?.oracle_webrtc_multiaddr
        if (!addr) {
            setHeartbeat("no oracle address")
            return
        }
        setPinging(true)
        setHeartbeat("dialing…")
        const result = await heartbeatOracle(addr)
        if (result.ok) {
            setHeartbeat(`PONG rtt=${result.rttMs?.toFixed(1)}ms`)
        } else {
            setHeartbeat(`failed: ${result.detail}`)
        }
        setPinging(false)
    }

    return (
        <aside className="sidebar animate-fade-in">
            {/* ── Daemon ── */}
            <div className="sidebar__section">
                <div className="sidebar__section-title">
                    <span className="sidebar__section-icon">⚡</span>
                    Daemon Status
                </div>
                <div className="stat-row">
                    <span className="stat-label">Rust Sidecar</span>
                    <span
                        className={`stat-value ${rustStatus === "connected"
                                ? "stat-value--accent"
                                : "stat-value--error"
                            }`}
                    >
                        {rustStatus}
                    </span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">Node Mode</span>
                    <span className="stat-value">
                        {mode === "local-oracle"
                            ? "Oracle"
                            : mode === "scout"
                                ? "Scout"
                                : mode}
                    </span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">API</span>
                    <span className="stat-value" style={{ fontSize: "10px" }}>
                        :8000
                    </span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">Control Plane</span>
                    <span className="stat-value" style={{ fontSize: "10px" }}>
                        :9091
                    </span>
                </div>
            </div>

            {/* ── Topology ── */}
            <div className="sidebar__section">
                <div className="sidebar__section-title">
                    <span className="sidebar__section-icon">🌐</span>
                    Topology
                </div>
                <div className="stat-row">
                    <span className="stat-label">Peer ID</span>
                    <span
                        className="stat-value"
                        title={topology?.oracle_peer_id ?? ""}
                        style={{ fontSize: "10px", maxWidth: "140px", overflow: "hidden", textOverflow: "ellipsis" }}
                    >
                        {topology?.oracle_peer_id
                            ? topology.oracle_peer_id.slice(0, 16) + "…"
                            : "—"}
                    </span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">WS Addr</span>
                    <span
                        className="stat-value"
                        style={{ fontSize: "9px", maxWidth: "140px", overflow: "hidden", textOverflow: "ellipsis" }}
                    >
                        {topology?.oracle_ws_multiaddr ? "✓ available" : "—"}
                    </span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">WebRTC Addr</span>
                    <span
                        className="stat-value"
                        style={{ fontSize: "9px", maxWidth: "140px", overflow: "hidden", textOverflow: "ellipsis" }}
                    >
                        {topology?.oracle_webrtc_multiaddr ? "✓ available" : "—"}
                    </span>
                </div>
            </div>

            {/* ── Peers ── */}
            <div className="sidebar__section">
                <div className="sidebar__section-title">
                    <span className="sidebar__section-icon">👥</span>
                    Connected Peers ({peers.length})
                </div>
                {peers.length === 0 ? (
                    <div
                        style={{
                            fontSize: "12px",
                            color: "var(--text-muted)",
                            textAlign: "center",
                            padding: "12px 0",
                        }}
                    >
                        No peers connected yet.
                        <br />
                        <span style={{ fontSize: "10px" }}>
                            Share your multiaddr to connect.
                        </span>
                    </div>
                ) : (
                    <ul className="peer-list">
                        {peers.map((p) => (
                            <li key={p.peer_id} className="peer-item">
                                <span className="status-dot status-dot--live" />
                                <span className="peer-item__id">{p.peer_id.slice(0, 20)}…</span>
                            </li>
                        ))}
                    </ul>
                )}
            </div>

            {/* ── Heartbeat ── */}
            <div className="sidebar__section">
                <div className="sidebar__section-title">
                    <span className="sidebar__section-icon">💓</span>
                    Heartbeat
                </div>
                <div
                    style={{
                        fontSize: "12px",
                        fontFamily: "var(--font-mono)",
                        color:
                            heartbeat.includes("PONG")
                                ? "var(--accent-emerald)"
                                : heartbeat.includes("fail")
                                    ? "var(--accent-rose)"
                                    : "var(--text-secondary)",
                        marginBottom: "10px",
                        minHeight: "18px",
                    }}
                >
                    {heartbeat}
                </div>
                <button
                    className="btn-ping"
                    onClick={doPing}
                    disabled={pinging}
                >
                    {pinging ? "Pinging…" : "Send PING"}
                </button>
            </div>
        </aside>
    )
}
