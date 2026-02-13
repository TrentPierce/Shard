"use client"

import { useEffect, useState } from "react"
import type { NodeMode } from "@/app/page"
import type { Topology } from "@/lib/swarm"
import { heartbeatShard } from "@/lib/swarm"
import type { ModelProgress } from "@/lib/webllm"
import { apiUrl } from "@/lib/config"

interface NetworkStatusProps {
    mode: NodeMode
    topology: Topology | null
    rustStatus: "connected" | "unreachable"
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
        const addr =
            topology?.shard_ws_multiaddr ?? topology?.shard_webrtc_multiaddr
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
        <aside className="sidebar animate-fade-in" role="complementary" aria-label="Network status and information">
            {/* ── Daemon ── */}
            <div className="sidebar__section">
                <div className="sidebar__section-title">
                    <span className="sidebar__section-icon" aria-hidden="true">⚡</span>
                    Daemon Status
                </div>
                <div className="stat-row">
                    <span className="stat-label" id="rust-status-label">Rust Sidecar</span>
                    <span
                        className={`stat-value ${rustStatus === "connected"
                                ? "stat-value--accent"
                                : "stat-value--error"
                            }`}
                        aria-labelledby="rust-status-label"
                    >
                        {rustStatus}
                    </span>
                </div>
                <div className="stat-row">
                    <span className="stat-label" id="node-mode-label">Node Mode</span>
                    <span className="stat-value" aria-labelledby="node-mode-label">
                        {mode === "local-shard"
                            ? "Shard"
                            : mode === "scout"
                                ? "Scout"
                                : mode === "scout-initializing"
                                    ? "Scout (Initializing…)"
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

            {/* ── WebLLM Status (Scout Mode) ── */}
            {(mode === "scout" || mode === "scout-initializing" || webLLMError) && (
                <div className="sidebar__section">
                    <div className="sidebar__section-title">
                        <span className="sidebar__section-icon">🧠</span>
                        WebLLM (Scout)
                    </div>
                    {webLLMError ? (
                        <div
                            style={{
                                fontSize: "11px",
                                color: "var(--accent-rose)",
                                marginBottom: "8px",
                                lineHeight: "1.4",
                            }}
                        >
                            {webLLMError}
                        </div>
                    ) : webLLMProgress ? (
                        <>
                            <div className="stat-row">
                                <span className="stat-label">Status</span>
                                <span className="stat-value">
                                    {Math.round(webLLMProgress.progress * 100)}%
                                </span>
                            </div>
                            <div
                                style={{
                                    fontSize: "10px",
                                    color: "var(--text-muted)",
                                    marginBottom: "6px",
                                    lineHeight: "1.3",
                                    maxHeight: "60px",
                                    overflow: "hidden",
                                }}
                            >
                                {webLLMProgress.text}
                            </div>
                            <div className="stat-row">
                                <span className="stat-label">Elapsed</span>
                                <span className="stat-value" style={{ fontSize: "10px" }}>
                                    {(webLLMProgress.timeElapsed / 1000).toFixed(1)}s
                                </span>
                            </div>
                        </>
                    ) : (
                        <div
                            style={{
                                fontSize: "11px",
                                color: "var(--accent-emerald)",
                            }}
                        >
                            ✓ Ready (Llama-3.2-1B)
                        </div>
                    )}
                </div>
            )}

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
                        title={topology?.shard_peer_id ?? ""}
                        style={{ fontSize: "10px", maxWidth: "140px", overflow: "hidden", textOverflow: "ellipsis" }}
                    >
                        {topology?.shard_peer_id
                            ? topology.shard_peer_id.slice(0, 16) + "…"
                            : "—"}
                    </span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">WS Addr</span>
                    <span
                        className="stat-value"
                        style={{ fontSize: "9px", maxWidth: "140px", overflow: "hidden", textOverflow: "ellipsis" }}
                    >
                        {topology?.shard_ws_multiaddr ? "✓ available" : "—"}
                    </span>
                </div>
                <div className="stat-row">
                    <span className="stat-label">WebRTC Addr</span>
                    <span
                        className="stat-value"
                        style={{ fontSize: "9px", maxWidth: "140px", overflow: "hidden", textOverflow: "ellipsis" }}
                    >
                        {topology?.shard_webrtc_multiaddr ? "✓ available" : "—"}
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

            {/* ── Queue Status (Leech Mode) ── */}
            {mode === "leech" && (
                <div className="sidebar__section">
                    <div className="sidebar__section-title">
                        <span className="sidebar__section-icon">⏳</span>
                        Queue Status
                    </div>
                    <div className="stat-row">
                        <span className="stat-label">Priority</span>
                        <span className="stat-value" style={{ color: "#f59e0b" }}>Low</span>
                    </div>
                    <div
                        style={{
                            fontSize: "11px",
                            color: "var(--text-muted)",
                            marginTop: "8px",
                            padding: "10px",
                            background: "rgba(245, 158, 11, 0.1)",
                            borderRadius: "6px",
                            border: "1px solid rgba(245, 158, 11, 0.2)",
                            lineHeight: "1.4",
                        }}
                    >
                        <strong style={{ color: "#f59e0b" }}>Leech Mode Active</strong>
                        <br />
                        You have lowest priority. Enable Scout mode to skip the queue and earn priority tokens.
                    </div>
                    <button
                        className="btn-ping"
                        style={{
                            background: "linear-gradient(135deg, #f59e0b, #d97706)",
                            marginTop: "12px",
                            width: "100%",
                        }}
                        onClick={() => {
                            window.location.reload();
                        }}
                    >
                        Enable Scout Mode
                    </button>
                </div>
            )}

            {/* ── Scout Status ── */}
            {(mode === "scout" || mode === "scout-initializing") && (
                <div className="sidebar__section">
                    <div className="sidebar__section-title">
                        <span className="sidebar__section-icon">🔍</span>
                        Scout Status
                    </div>
                    <div className="stat-row">
                        <span className="stat-label">Priority</span>
                        <span className="stat-value stat-value--accent">High</span>
                    </div>
                    <div className="stat-row">
                        <span className="stat-label">Contribution</span>
                        <span className="stat-value" style={{ color: "#10b981" }}>Active</span>
                    </div>
                    <div
                        style={{
                            fontSize: "11px",
                            color: "var(--text-muted)",
                            marginTop: "8px",
                            padding: "10px",
                            background: "rgba(16, 185, 129, 0.1)",
                            borderRadius: "6px",
                            border: "1px solid rgba(16, 185, 129, 0.2)",
                            lineHeight: "1.4",
                        }}
                    >
                        <strong style={{ color: "#10b981" }}>Scout Mode Active</strong>
                        <br />
                        Your browser is generating draft tokens and contributing to the network. You have high priority access.
                    </div>
                </div>
            )}

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
                    role="status"
                    aria-live="polite"
                    aria-atomic="true"
                    id="heartbeat-status"
                >
                    {heartbeat}
                </div>
                <button
                    className="btn-ping"
                    onClick={doPing}
                    disabled={pinging}
                    type="button"
                    aria-label="Send network ping"
                    aria-describedby="heartbeat-status"
                >
                    {pinging ? "Pinging…" : "Send PING"}
                </button>
            </div>
        </aside>
    )
}
