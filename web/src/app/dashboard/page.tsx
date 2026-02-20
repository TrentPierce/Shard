'use client'

import React, { useState, useEffect, useCallback } from 'react'
import { TopologyGraph } from '@/components/dashboard/TopologyGraph'
import { MeshHealthGauge } from '@/components/dashboard/MeshHealthGauge'
import { TPSChart } from '@/components/dashboard/TPSChart'
import { DropoffChart } from '@/components/dashboard/DropoffChart'
import { AlertFeed } from '@/components/dashboard/AlertFeed'

interface TelemetryData {
    peers: Record<string, PeerNode>
    metrics: SystemMetrics
    alerts: AlertEvent[]
    topology: TopologyEdge[]
    tpsHistory: TPSSample[]
    dropoffHistory: DropoffSample[]
}

interface PeerNode {
    peerId: string
    role: 'Verifier' | 'Scout'
    latencyMs: number
    tokensPerSecond: number
    isHealthy: boolean
    lastSeen: number
}

interface SystemMetrics {
    totalTps: number
    avgLatencyMs: number
    peerCount: number
    sigFailureRate: number
    uptime: number
    scoutDropoffs: number
    powChallengesIssued: number
    powChallengesFailed: number
    privateRouteRequests: number
    fallbackInvocations: number
}

interface AlertEvent {
    id: string
    kind: 'ddos' | 'latency_spike' | 'sig_failure' | 'sybil' | 'info'
    message: string
    severity: 'critical' | 'warning' | 'info'
    timestamp: number
}

interface TopologyEdge {
    source: string
    target: string
    latencyMs: number
    healthy: boolean
}

interface TPSSample {
    timestamp: number
    value: number
}

interface DropoffSample {
    timestamp: number
    count: number
}

const POLL_INTERVAL = 2000

export default function DashboardPage() {
    const [data, setData] = useState<TelemetryData | null>(null)
    const [connected, setConnected] = useState(false)

    const fetchTelemetry = useCallback(async () => {
        try {
            const [statusRes, metricsRes] = await Promise.all([
                fetch('http://localhost:8080/status'),
                fetch('http://localhost:8080/metrics'),
            ])

            if (!statusRes.ok || !metricsRes.ok) {
                setConnected(false)
                return
            }

            const status = await statusRes.json()
            const metricsText = await metricsRes.text()

            // Parse peers
            const peers: Record<string, PeerNode> = {}
            if (status.peers) {
                for (const [id, info] of Object.entries(status.peers) as [string, any][]) {
                    peers[id] = {
                        peerId: id,
                        role: info.role || 'Scout',
                        latencyMs: info.latency_ms || 0,
                        tokensPerSecond: info.tps || 0,
                        isHealthy: info.healthy !== false,
                        lastSeen: info.last_seen || Date.now(),
                    }
                }
            }

            // Parse metrics from prometheus text
            const parseMetric = (name: string): number => {
                const match = metricsText.match(new RegExp(`^${name}\\s+([\\d.]+)`, 'm'))
                return match ? parseFloat(match[1]) : 0
            }

            const metrics: SystemMetrics = {
                totalTps: parseMetric('shard_tokens_processed_total'),
                avgLatencyMs: parseMetric('shard_node_latency_ms'),
                peerCount: parseMetric('shard_active_node_count'),
                sigFailureRate: parseMetric('shard_signature_verification_failures_total'),
                uptime: parseMetric('shard_node_uptime_seconds'),
                scoutDropoffs: parseMetric('shard_scout_dropoff_total'),
                powChallengesIssued: parseMetric('shard_pow_challenges_issued_total'),
                powChallengesFailed: parseMetric('shard_pow_challenges_failed_total'),
                privateRouteRequests: parseMetric('shard_private_route_total'),
                fallbackInvocations: parseMetric('shard_fallback_invocations_total'),
            }

            // Build topology edges from peer list
            const localId = status.topology?.local_peer_id || 'local'
            const topology: TopologyEdge[] = Object.values(peers).map((p) => ({
                source: localId,
                target: p.peerId,
                latencyMs: p.latencyMs,
                healthy: p.isHealthy,
            }))

            // Append to history
            const now = Date.now()
            setData((prev) => ({
                peers,
                metrics,
                alerts: status.alerts || prev?.alerts || [],
                topology,
                tpsHistory: [
                    ...(prev?.tpsHistory || []).slice(-60),
                    { timestamp: now, value: metrics.totalTps },
                ],
                dropoffHistory: [
                    ...(prev?.dropoffHistory || []).slice(-60),
                    { timestamp: now, count: metrics.scoutDropoffs },
                ],
            }))
            setConnected(true)
        } catch {
            setConnected(false)
        }
    }, [])

    useEffect(() => {
        fetchTelemetry()
        const interval = setInterval(fetchTelemetry, POLL_INTERVAL)
        return () => clearInterval(interval)
    }, [fetchTelemetry])

    return (
        <main className="dashboard-page">
            <div className="dashboard-header">
                <h1>Shard Network Dashboard</h1>
                <div className={`connection-status ${connected ? 'online' : 'offline'}`}>
                    <span className="status-dot" />
                    {connected ? 'Connected' : 'Disconnected'}
                </div>
            </div>

            <div className="dashboard-grid">
                {/* Top row — health gauges */}
                <div className="dashboard-card gauge-card">
                    <h2>Mesh Health</h2>
                    <MeshHealthGauge
                        peerCount={data?.metrics.peerCount || 0}
                        avgLatency={data?.metrics.avgLatencyMs || 0}
                        sigFailureRate={data?.metrics.sigFailureRate || 0}
                    />
                </div>

                <div className="dashboard-card stats-card">
                    <h2>Key Metrics</h2>
                    <div className="stats-grid">
                        <div className="stat">
                            <span className="stat-value">{data?.metrics.totalTps.toLocaleString() || '0'}</span>
                            <span className="stat-label">Total Tokens</span>
                        </div>
                        <div className="stat">
                            <span className="stat-value">{data?.metrics.peerCount || 0}</span>
                            <span className="stat-label">Active Peers</span>
                        </div>
                        <div className="stat">
                            <span className="stat-value">{data?.metrics.scoutDropoffs || 0}</span>
                            <span className="stat-label">Scout Dropoffs</span>
                        </div>
                        <div className="stat">
                            <span className="stat-value">{data?.metrics.powChallengesIssued || 0}</span>
                            <span className="stat-label">PoW Challenges</span>
                        </div>
                        <div className="stat">
                            <span className="stat-value">{data?.metrics.privateRouteRequests || 0}</span>
                            <span className="stat-label">Private Routes</span>
                        </div>
                        <div className="stat">
                            <span className="stat-value">{data?.metrics.fallbackInvocations || 0}</span>
                            <span className="stat-label">Fallbacks</span>
                        </div>
                    </div>
                </div>

                {/* Middle row — charts */}
                <div className="dashboard-card chart-card">
                    <h2>Token Throughput</h2>
                    <TPSChart data={data?.tpsHistory || []} />
                </div>

                <div className="dashboard-card chart-card">
                    <h2>Scout Dropoffs</h2>
                    <DropoffChart data={data?.dropoffHistory || []} />
                </div>

                {/* Bottom row — topology + alerts */}
                <div className="dashboard-card topology-card">
                    <h2>Network Topology</h2>
                    <TopologyGraph
                        peers={data ? Object.values(data.peers) : []}
                        edges={data?.topology || []}
                    />
                </div>

                <div className="dashboard-card alerts-card">
                    <h2>Alert Feed</h2>
                    <AlertFeed alerts={data?.alerts || []} />
                </div>
            </div>

            <style jsx>{`
        .dashboard-page {
          padding: 24px;
          max-width: 1400px;
          margin: 0 auto;
          font-family: 'Inter', -apple-system, sans-serif;
        }
        .dashboard-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 24px;
        }
        .dashboard-header h1 {
          font-size: 28px;
          font-weight: 700;
          background: linear-gradient(135deg, #7c83ff, #a78bfa);
          -webkit-background-clip: text;
          -webkit-text-fill-color: transparent;
        }
        .connection-status {
          display: flex;
          align-items: center;
          gap: 8px;
          font-size: 13px;
          font-weight: 500;
          padding: 6px 14px;
          border-radius: 20px;
          background: rgba(255,255,255,0.05);
          border: 1px solid rgba(255,255,255,0.1);
        }
        .connection-status.online { color: #4ade80; }
        .connection-status.offline { color: #f87171; }
        .status-dot {
          width: 8px;
          height: 8px;
          border-radius: 50%;
          background: currentColor;
          animation: pulse 2s infinite;
        }
        @keyframes pulse {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.4; }
        }
        .dashboard-grid {
          display: grid;
          grid-template-columns: 1fr 1fr;
          gap: 20px;
        }
        .dashboard-card {
          background: rgba(255,255,255,0.03);
          border: 1px solid rgba(255,255,255,0.08);
          border-radius: 16px;
          padding: 20px;
          backdrop-filter: blur(10px);
        }
        .dashboard-card h2 {
          font-size: 14px;
          font-weight: 600;
          text-transform: uppercase;
          letter-spacing: 0.05em;
          color: rgba(255,255,255,0.6);
          margin-bottom: 16px;
        }
        .stats-grid {
          display: grid;
          grid-template-columns: repeat(3, 1fr);
          gap: 16px;
        }
        .stat {
          text-align: center;
          padding: 12px;
          background: rgba(255,255,255,0.03);
          border-radius: 12px;
        }
        .stat-value {
          display: block;
          font-size: 24px;
          font-weight: 700;
          color: #a78bfa;
          font-variant-numeric: tabular-nums;
        }
        .stat-label {
          display: block;
          font-size: 11px;
          color: rgba(255,255,255,0.5);
          margin-top: 4px;
          text-transform: uppercase;
          letter-spacing: 0.05em;
        }
        .topology-card {
          min-height: 300px;
        }
        .alerts-card {
          max-height: 400px;
          overflow-y: auto;
        }
        @media (max-width: 768px) {
          .dashboard-grid {
            grid-template-columns: 1fr;
          }
        }
      `}</style>
        </main>
    )
}
