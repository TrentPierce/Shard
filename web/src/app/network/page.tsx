"use client"

import { useMemo } from "react"
import Header from "@/components/Header"
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
    <div className="container section">
      <Header />
      <main>
        <div>
          <section className="mb-0 text-center">
            <p className="text-secondary text-mono mb-0">Operations Console</p>
            <h1 className="mb-0">Shard Telemetry</h1>
            <p className="text-secondary mb-xl">
              Real-time network health for contributors, operators, and API consumers.
              Distributed performance improves as both Scout and Shard participation rises.
            </p>
            <div className="flex justify-center gap-md mt-auto mb-0" style={{ flexWrap: 'wrap' }}>
              <span className="badge badge-primary">{statusLabel}</span>
              <span className="badge badge-secondary">Model: shard-hybrid</span>
              <span className="badge badge-secondary">Sessions: {analytics.sessions}</span>
              <span className="badge badge-secondary">Active scouts: {health.active_scouts ?? 0}</span>
              <span className="badge badge-secondary">Success: {successRate}%</span>
              <span className="badge badge-secondary">
                Avg latency: {analytics.avgLatencyMs > 0 ? `${analytics.avgLatencyMs}ms` : "n/a"}
              </span>
              <span className="badge badge-secondary">
                Last incident: {health.last_incident && health.last_incident !== "none" ? health.last_incident : "none"}
              </span>
              <a className="btn btn-ghost btn-sm" href="https://github.com/TrentPierce/Shard/blob/main/docs/join-network.md" target="_blank" rel="noreferrer">
                How to contribute
              </a>
            </div>
          </section>

          <section className="grid grid-3 mt-auto">
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

          <section className="grid grid-2 mt-auto">
            <div className="card" style={{ gridColumn: '1 / -1' }}>
              <div className="mb-0">
                <h2 className="text-mono">Throughput Timeline</h2>
                <span className="text-secondary">last {telemetry.throughputHistory.length} samples</span>
              </div>
              <SwarmThroughputCanvas samples={telemetry.throughputHistory} />
            </div>

            <div className="card">
              <div className="mb-0">
                <h2 className="text-mono">Node Mix</h2>
                <span className="text-secondary">{compactNumber(totalNodes)} active nodes</span>
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
        </div>
      </main>
    </div>
  )
}

