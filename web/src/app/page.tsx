"use client"

import Link from "next/link"
import Image from "next/image"
import { useEffect, useState } from "react"
import { useDashboardTelemetry } from "@/hooks/useDashboardTelemetry"
import { apiUrl } from "@/lib/config"
import { probeWebGPU, type WebGPUProbeResult } from "@/lib/webgpu-probe"

const telemetryCards = [
  { key: "verifier", label: "Active Verifier Nodes" },
  { key: "scouts", label: "Active Browser Scouts" },
  { key: "tps", label: "Tokens / Second" },
  { key: "total", label: "Total Tokens Generated" },
] as const

const flow = [
  {
    title: "Prompt enters the mesh",
    body: "Clients submit requests over OpenAI-compatible endpoints to active verifiers.",
  },
  {
    title: "Scouts draft candidate tokens",
    body: "Browser workers generate fast drafts locally and forward them into the network.",
  },
  {
    title: "Verifiers confirm and stream",
    body: "Verifier nodes validate drafts and stream trusted tokens back with low latency.",
  },
]

const quickStart = [
  {
    title: "Scout (No Install)",
    bullets: [
      "Open shardnetwork.live",
      "Click Join / Start Contributing",
      "Keep tab open to contribute WebGPU compute",
    ],
  },
  {
    title: "Verifier (Docker)",
    bullets: [
      "git clone github.com/TrentPierce/Shard",
      "docker compose up --build shard-daemon -d",
      "curl localhost:9091/health",
    ],
  },
]

const incentives = [
  "Compute-for-compute model: contribute capacity, draw capacity when needed.",
  "No token required to start participating in current contribution flow.",
  "Practical benefits: lower API spend, burst scaling, infrastructure ownership.",
]

const benchmarkRows = [
  {
    scenario: "1 node, no scouts",
    p95Ms: 4523.11,
    tps: 4.0333,
    errorPct: 0.0,
    speculativeSamples: 0,
  },
  {
    scenario: "1 node, with scouts",
    p95Ms: 10016.93,
    tps: 3.8,
    errorPct: 5.7851,
    speculativeSamples: 36,
  },
  {
    scenario: "2 nodes, no scouts",
    p95Ms: 3031.208,
    tps: 4.0333,
    errorPct: 0.0,
    speculativeSamples: 0,
  },
  {
    scenario: "2 nodes, with scouts",
    p95Ms: 7719.465,
    tps: 4.0,
    errorPct: 0.8264,
    speculativeSamples: 12,
  },
]

const oldVsNew = [
  "2-node no-scouts vs previous published matrix: p95 latency -40.32%, throughput +14.15%, errors -11.30 points.",
  "2-node with scouts vs previous published matrix: p95 latency +27.14%, throughput +10.09%, errors -7.96 points.",
  "1-node no-scouts vs previous published matrix: p95 latency -10.39%, throughput +16.91%, errors -13.39 points.",
  "1-node with scouts vs previous published matrix: p95 latency +253.09%, throughput -3.39%, errors +4.53 points.",
]

export default function HomePage() {
  const telemetry = useDashboardTelemetry()
  const [probeResult, setProbeResult] = useState<WebGPUProbeResult | null>(null)
  const statusClass =
    telemetry.healthState === "ready"
      ? "border-accent-400/60 bg-accent-400/15 text-ink-50"
      : telemetry.healthState === "degraded"
      ? "border-base-700/70 bg-base-700/30 text-ink-100"
      : "border-base-800/80 bg-base-900/70 text-ink-300"

  const values = {
    verifier: telemetry.verifierNodes.toLocaleString(),
    scouts: telemetry.scouts.toLocaleString(),
    tps: telemetry.tokensPerSecond.toLocaleString(),
    total: telemetry.totalTokensGenerated.toLocaleString(),
  }

  useEffect(() => {
    let cancelled = false
    async function runProbe() {
      try {
        const result = await probeWebGPU()
        if (cancelled) return
        setProbeResult(result)
        await fetch(apiUrl("/v1/telemetry/webgpu"), {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(result),
          keepalive: true,
        })
      } catch {
        if (!cancelled) {
          setProbeResult({
            eligible: false,
            reason: "probe_error",
            tier: "none",
            estimated_vram_mb: 0,
            supports_f16: false,
            browser: "Unknown",
            os: "Unknown",
            adapter_vendor: "unknown",
            adapter_device: "unknown",
          })
        }
      }
    }

    void runProbe()
    return () => {
      cancelled = true
    }
  }, [])

  return (
    <>
      <main id="main-content" className="pb-16 pt-10 sm:pt-14">
        <section className="relative overflow-hidden rounded-3xl border border-ring bg-gradient-to-br from-base-900 to-base-800 px-6 py-12 shadow-panel sm:px-10 sm:py-16">
          <div className="relative mx-auto max-w-4xl">
            {probeResult && (
              <div
                className={`mb-4 inline-flex rounded-full border px-3 py-1 text-xs font-medium ${
                  probeResult.eligible
                    ? "border-accent-400/60 bg-accent-400/15 text-ink-50"
                    : "border-ring/70 bg-base-900/70 text-ink-200"
                }`}
              >
                {probeResult.eligible
                  ? "Your browser is contributing compute"
                  : "Your browser is in viewer mode (WebGPU not available)"}
              </div>
            )}
            <div className="mb-6 flex justify-center sm:justify-start">
              <Image
                src="/icon-192.png"
                alt="Shard Network"
                width={80}
                height={80}
                className="h-16 w-16 rounded-2xl sm:h-20 sm:w-20"
                priority
              />
            </div>
            <div className="mb-5 flex flex-wrap items-center gap-3">
              <p className="text-xs font-semibold uppercase tracking-[0.22em] text-accent-400">
                Shard 0.6.2 | Distributed Inference Mesh
              </p>
              <span
                className={`inline-flex min-h-7 items-center rounded-full border px-2.5 text-[11px] font-semibold uppercase tracking-[0.14em] ${statusClass}`}
              >
                {telemetry.statusLabel}
              </span>
            </div>
            <h1 className="text-balance text-3xl font-semibold tracking-tight text-ink-50 sm:text-5xl">
              Reduce AI cost and scale with browser scouts + verifier nodes.
            </h1>
            <p className="mt-5 max-w-2xl text-pretty text-base text-ink-300 sm:text-lg">
              In under 10 seconds: Shard is an OpenAI-compatible distributed inference network where scouts draft,
              verifiers validate, and operators keep ownership of runtime + policy.
            </p>
            <div className="mt-8 flex flex-col gap-3 sm:flex-row sm:items-center">
              <Link
                href="/start"
                className="inline-flex min-h-11 items-center justify-center rounded-lg bg-accent-500 px-5 py-2.5 text-sm font-medium text-base-950 transition hover:bg-accent-400"
              >
                Join as Scout
              </Link>
              <Link
                href="/chat"
                className="inline-flex min-h-11 items-center justify-center rounded-lg border border-ring px-5 py-2.5 text-sm font-medium text-ink-100 transition hover:bg-base-800"
              >
                Test Verifier API
              </Link>
            </div>
          </div>
        </section>

        <section className="mt-10">
          <h2 className="text-balance text-xl font-medium text-ink-50 sm:text-2xl">Start Contributing Fast</h2>
          <div className="mt-4 grid gap-3 md:grid-cols-2">
            {quickStart.map((card) => (
              <article key={card.title} className="rounded-2xl border border-ring bg-base-900 p-5">
                <h3 className="text-lg font-medium text-ink-50">{card.title}</h3>
                <ul className="mt-3 list-disc space-y-1 pl-5 text-sm text-ink-300">
                  {card.bullets.map((item) => (
                    <li key={item}>{item}</li>
                  ))}
                </ul>
              </article>
            ))}
          </div>
        </section>

        <section className="mt-10">
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-balance text-xl font-medium text-ink-50 sm:text-2xl">Live Telemetry</h2>
            <p className="text-xs uppercase tracking-[0.18em] text-ink-400">
              {telemetry.isLive ? "Live from network" : "Fallback simulation"}
            </p>
          </div>
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
            {telemetryCards.map((card) => (
              <article key={card.key} className="rounded-2xl border border-ring bg-base-900 p-5">
                <p className="text-sm text-ink-300">{card.label}</p>
                <p className="mt-3 text-2xl font-semibold text-ink-50 sm:text-3xl">{values[card.key]}</p>
              </article>
            ))}
          </div>
        </section>

        <section className="mt-12">
          <h2 className="text-balance text-xl font-medium text-ink-50 sm:text-2xl">March 5, 2026 Benchmark Snapshot</h2>
          <p className="mt-2 text-sm text-ink-300">
            Isolated scenario runs from today using the distributed orchestrator (24 scouts, 4 req/s, 30s runs).
            Local + EC2 verifiers were restarted before each scenario to avoid queue carryover between cases.
          </p>
          <div className="mt-4 overflow-x-auto rounded-2xl border border-ring bg-base-900">
            <table className="min-w-full text-left text-sm">
              <thead className="border-b border-ring text-ink-300">
                <tr>
                  <th className="px-4 py-3 font-medium">Scenario</th>
                  <th className="px-4 py-3 font-medium">p95 Latency (ms)</th>
                  <th className="px-4 py-3 font-medium">Throughput (TPS)</th>
                  <th className="px-4 py-3 font-medium">Error Rate (%)</th>
                  <th className="px-4 py-3 font-medium">Speculative Samples</th>
                </tr>
              </thead>
              <tbody>
                {benchmarkRows.map((row) => (
                  <tr key={row.scenario} className="border-b border-ring/60 text-ink-100">
                    <td className="px-4 py-3">{row.scenario}</td>
                    <td className="px-4 py-3">{row.p95Ms.toLocaleString(undefined, { maximumFractionDigits: 3 })}</td>
                    <td className="px-4 py-3">{row.tps.toLocaleString(undefined, { maximumFractionDigits: 4 })}</td>
                    <td className="px-4 py-3">{row.errorPct.toLocaleString(undefined, { maximumFractionDigits: 4 })}</td>
                    <td className="px-4 py-3">{row.speculativeSamples.toLocaleString()}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <ul className="mt-4 list-disc space-y-1 pl-5 text-sm text-ink-300">
            {oldVsNew.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </section>

        <section className="mt-12">
          <h2 className="text-balance text-xl font-medium text-ink-50 sm:text-2xl">Participation Incentives</h2>
          <ul className="mt-4 list-disc space-y-2 pl-5 text-sm text-ink-300">
            {incentives.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </section>

        <section className="mt-12">
          <h2 className="text-balance text-xl font-medium text-ink-50 sm:text-2xl">How it Works</h2>
          <div className="mt-4 grid gap-3 md:grid-cols-3">
            {flow.map((item, index) => (
              <article key={item.title} className="rounded-2xl border border-ring bg-base-900 p-5">
                <p className="text-xs uppercase tracking-[0.18em] text-accent-400">Step {index + 1}</p>
                <h3 className="mt-2 text-lg font-medium text-ink-50">{item.title}</h3>
                <p className="mt-2 text-sm text-ink-300">{item.body}</p>
              </article>
            ))}
          </div>
        </section>
      </main>
    </>
  )
}
