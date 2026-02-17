"use client"

import { useMemo } from "react"
import TelemetryStatCard from "@/components/network/TelemetryStatCard"
import TopContributorsTable from "@/components/network/TopContributorsTable"
import SwarmThroughputCanvas from "@/components/network/SwarmThroughputCanvas"
import { useSwarmTelemetry } from "@/hooks/useSwarmTelemetry"
import { useProductSignals } from "@/hooks/useProductSignals"

const compactNumber = (value: number) =>
  new Intl.NumberFormat("en-US", { maximumFractionDigits: 2 }).format(value)

export default function NetworkTelemetryPage() {
  const { telemetry, statusLabel } = useSwarmTelemetry()
  const { health, analytics, successRate } = useProductSignals()

  const totalNodes = useMemo(
    () => telemetry.scoutCount + telemetry.shardCount,
    [telemetry.scoutCount, telemetry.shardCount],
  )

  const scoutRatio = totalNodes > 0 ? (telemetry.scoutCount / totalNodes) * 100 : 0

  return (
    <main className="network-page">
      <div className="network-page__noise" aria-hidden />
      <section className="network-page__hero">
        <p className="network-page__kicker">Operations Console</p>
        <h1>Shard Telemetry</h1>
        <p className="network-page__summary">
          Real-time network health for contributors, operators, and API consumers.
          Distributed performance improves as both Scout and Shard participation rises.
        </p>
        <div className="network-page__hero-meta">
          <span className="network-page__badge">{statusLabel}</span>
          <span className="network-page__badge">Model: shard-hybrid</span>
          <span className="network-page__badge">Sessions: {analytics.sessions}</span>
          <span className="network-page__badge">Success: {successRate}%</span>
          <span className="network-page__badge">
            Avg latency: {analytics.avgLatencyMs > 0 ? `${analytics.avgLatencyMs}ms` : "n/a"}
          </span>
          <span className="network-page__badge">
            Last incident: {health.last_incident && health.last_incident !== "none" ? health.last_incident : "none"}
          </span>
          <a className="network-page__link" href="https://github.com/TrentPierce/Shard/blob/main/docs/join-network.md" target="_blank" rel="noreferrer">
            How to contribute
          </a>
        </div>
      </section>

      <section className="network-grid network-grid--stats">
        <TelemetryStatCard
          label="Global Swarm TFLOPs"
          value={`${compactNumber(telemetry.globalTflops)} TFLOPs`}
          hint="rolling estimator from active compute peers"
          accent="cyan"
        />
        <TelemetryStatCard
          label="Active WebGPU Scouts"
          value={compactNumber(telemetry.scoutCount)}
          hint="browser draft generators"
          accent="violet"
        />
        <TelemetryStatCard
          label="Active Desktop Shards"
          value={compactNumber(telemetry.shardCount)}
          hint="full-model verifiers"
          accent="emerald"
        />
      </section>

      <section className="network-grid network-grid--main">
        <div className="network-card network-card--wide">
          <div className="network-card__header">
            <h2>Throughput Timeline</h2>
            <span>last {telemetry.throughputHistory.length} samples</span>
          </div>
          <SwarmThroughputCanvas samples={telemetry.throughputHistory} />
        </div>

        <div className="network-card">
          <div className="network-card__header">
            <h2>Node Mix</h2>
            <span>{compactNumber(totalNodes)} active nodes</span>
          </div>
          <div className="node-mix">
            <div
              className="node-mix__donut"
              style={{
                background: `conic-gradient(#64748b 0% ${scoutRatio.toFixed(2)}%, #22c55e ${scoutRatio.toFixed(2)}% 100%)`,
              }}
              aria-label="Donut chart showing scouts and shards"
            >
              <div className="node-mix__center">
                <strong>{Math.round(scoutRatio)}%</strong>
                <span>Scouts</span>
              </div>
            </div>
            <div className="node-mix__legend">
              <p><span className="dot dot--violet" />Scouts - {telemetry.scoutCount}</p>
              <p><span className="dot dot--emerald" />Shards - {telemetry.shardCount}</p>
            </div>
          </div>
        </div>
      </section>

      <TopContributorsTable contributors={telemetry.contributors} />
    </main>
  )
}

