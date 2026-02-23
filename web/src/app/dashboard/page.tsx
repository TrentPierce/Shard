'use client'

import React, { useState, useEffect, useCallback } from 'react'
import Header from '@/components/Header'
import { TopologyGraph } from '@/components/dashboard/TopologyGraph'
import { MeshHealthGauge } from '@/components/dashboard/MeshHealthGauge'
import { TPSChart } from '@/components/dashboard/TPSChart'
import { DropoffChart } from '@/components/dashboard/DropoffChart'
import { AlertFeed } from '@/components/dashboard/AlertFeed'
import { apiUrl } from "@/lib/config"

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
                fetch(apiUrl('/v1/system/topology')),
                fetch(apiUrl('/metrics')),
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
        <div className="container section">
            <Header />
            <main>
                <div>
                    <section className="mb-0 text-center">
                        <p className="text-secondary text-mono mb-0">Node Performance</p>
                        <h1 className="mb-0">Network Dashboard</h1>
                        <div className="flex justify-center gap-md mt-auto mb-0" style={{ flexWrap: 'wrap' }}>
                            <span className={`badge ${connected ? 'badge-primary' : 'badge-secondary'}`}>
                                {connected ? 'CONNECTED' : 'OFFLINE'}
                            </span>
                            <span className="badge badge-secondary">Uptime: {data ? Math.floor(data.metrics.uptime / 3600) : 0}h</span>
                            <span className="badge badge-secondary">Peers: {data?.metrics.peerCount || 0}</span>
                            <span className="badge badge-secondary">Failure Rate: {data?.metrics.sigFailureRate || 0}%</span>
                        </div>
                    </section>

                    <div className="grid grid-2 mt-auto">
                        <div className="card">
                            <div className="mb-0">
                                <h3 className="text-mono">Mesh Health</h3>
                            </div>
                            <MeshHealthGauge
                                peerCount={data?.metrics.peerCount || 0}
                                avgLatency={data?.metrics.avgLatencyMs || 0}
                                sigFailureRate={data?.metrics.sigFailureRate || 0}
                            />
                        </div>

                        <div className="card">
                            <div className="mb-0">
                                <h3 className="text-mono">Key Metrics</h3>
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

                        <div className="card" style={{ gridColumn: '1 / -1' }}>
                            <div className="mb-0">
                                <h3 className="text-mono">Token Throughput</h3>
                            </div>
                            <TPSChart data={data?.tpsHistory || []} />
                        </div>

                        <div className="card">
                            <div className="mb-0">
                                <h3 className="text-mono">Scout Dropoffs</h3>
                            </div>
                            <DropoffChart data={data?.dropoffHistory || []} />
                        </div>

                        <div className="card" style={{ gridColumn: '1 / -1' }}>
                            <div className="mb-0">
                                <h3 className="text-mono">Network Topology</h3>
                            </div>
                            <div style={{ height: '400px' }}>
                                <TopologyGraph
                                    peers={data ? Object.values(data.peers) : []}
                                    edges={data?.topology || []}
                                />
                            </div>
                        </div>

                        <div className="card" style={{ gridColumn: '1 / -1' }}>
                            <div className="mb-0">
                                <h3 className="text-mono">Alert Feed</h3>
                            </div>
                            <AlertFeed alerts={data?.alerts || []} />
                        </div>
                    </div>
                </div>
            </main>
        </div>
    )
}
