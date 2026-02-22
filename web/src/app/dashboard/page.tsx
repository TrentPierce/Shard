'use client'

import React, { useState, useEffect, useCallback } from 'react'
import Header from '@/components/Header'
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
                fetch('/api/v1/system/topology'),
                fetch('/api/health'),
            ])

            if (!statusRes.ok || !metricsRes.ok) {
                setConnected(false)
                return
            }

            const status = await statusRes.json()
            const metricsText = await metricsRes.text()

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

            const localId = status.topology?.local_peer_id || 'local'
            const topology: TopologyEdge[] = Object.values(peers).map((p) => ({
                source: localId,
                target: p.peerId,
                latencyMs: p.latencyMs,
                healthy: p.isHealthy,
            }))

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
        <div className="app-shell">
            <Header />
            <main className="network-page">
                <div className="network-page__noise" aria-hidden />
                <div className="app-container">
                    <section className="network-page__hero">
                        <p className="network-page__kicker">Node Performance</p>
                        <h1>Network Dashboard</h1>
                        <div className="network-page__hero-meta" style={{ display: 'flex', flexWrap: 'wrap', gap: '8px' }}>
                            <span className={`network-page__badge ${connected ? 'status-dot--live' : 'status-dot--dead'}`}>
                                {connected ? 'CONNECTED' : 'OFFLINE'}
                            </span>
                            <span className="network-page__badge">Uptime: {data ? Math.floor(data.metrics.uptime / 3600) : 0}h</span>
                            <span className="network-page__badge">Peers: {data?.metrics.peerCount || 0}</span>
                            <span className="network-page__badge">Failure Rate: {data?.metrics.sigFailureRate || 0}%</span>
                        </div>
                    </section>

                    <div className="network-grid network-grid--main">
                        <div className="network-card">
                            <div className="network-card__header">
                                <h2>Mesh Health</h2>
                            </div>
                            <MeshHealthGauge
                                peerCount={data?.metrics.peerCount || 0}
                                avgLatency={data?.metrics.avgLatencyMs || 0}
                                sigFailureRate={data?.metrics.sigFailureRate || 0}
                            />
                        </div>

                        <div className="network-card">
                            <div className="network-card__header">
                                <h2>Key Metrics</h2>
                            </div>
                            <div className="leaderboard">
                                <div className="leaderboard__row">
                                    <div className="leaderboard__identity">
                                        <strong>{data?.metrics.totalTps.toLocaleString() || '0'}</strong>
                                        <small>Total Tokens</small>
                                    </div>
                                </div>
                                <div className="leaderboard__row">
                                    <div className="leaderboard__identity">
                                        <strong>{data?.metrics.scoutDropoffs || 0}</strong>
                                        <small>Scout Dropoffs</small>
                                    </div>
                                </div>
                                <div className="leaderboard__row">
                                    <div className="leaderboard__identity">
                                        <strong>{data?.metrics.powChallengesIssued || 0}</strong>
                                        <small>PoW Challenges</small>
                                    </div>
                                </div>
                            </div>
                        </div>

                        <div className="network-card network-card--wide">
                            <div className="network-card__header">
                                <h2>Token Throughput</h2>
                            </div>
                            <TPSChart data={data?.tpsHistory || []} />
                        </div>

                        <div className="network-card">
                            <div className="network-card__header">
                                <h2>Scout Dropoffs</h2>
                            </div>
                            <DropoffChart data={data?.dropoffHistory || []} />
                        </div>

                        <div className="network-card network-card--wide">
                            <div className="network-card__header">
                                <h2>Network Topology</h2>
                            </div>
                            <div style={{ height: '400px' }}>
                                <TopologyGraph
                                    peers={data ? Object.values(data.peers) : []}
                                    edges={data?.topology || []}
                                />
                            </div>
                        </div>

                        <div className="network-card">
                            <div className="network-card__header">
                                <h2>Alert Feed</h2>
                            </div>
                            <AlertFeed alerts={data?.alerts || []} />
                        </div>
                    </div>
                </div>
            </main>
        </div>
    )
}
