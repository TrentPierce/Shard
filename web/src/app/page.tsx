"use client"

import Link from "next/link"
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

const charts = [
  {
    title: "Validated p95 Response Time by Node Count",
    src: "/value-dashboard/performance-vs-nodes.png",
    alt: "Performance chart showing validated node-count latencies with pending counts marked",
  },
  {
    title: "Network Contribution Map (Test Data)",
    src: "/value-dashboard/network-map.png",
    alt: "World contribution map with sample regional scout and verifier nodes",
  },
  {
    title: "Cost Comparison per 1K Tokens",
    src: "/value-dashboard/cost-comparison.png",
    alt: "Cost chart comparing shard contribution mode with centralized API pricing",
  },
]

export default function HomePage() {
  const telemetry = useDashboardTelemetry()
  const [probeResult, setProbeResult] = useState<WebGPUProbeResult | null>(null)
  const statusClass =
    telemetry.healthState === "ready"
      ? "border-emerald-500/40 bg-emerald-500/10 text-emerald-300"
      : telemetry.healthState === "degraded"
      ? "border-amber-500/40 bg-amber-500/10 text-amber-300"
      : "border-rose-500/40 bg-rose-500/10 text-rose-300"

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
                    ? "border-emerald-500/40 bg-emerald-500/10 text-emerald-300"
                    : "border-slate-500/40 bg-slate-500/10 text-slate-200"
                }`}
              >
                {probeResult.eligible
                  ? "Your browser is contributing compute"
                  : "Your browser is in viewer mode (WebGPU not available)"}
              </div>
            )}
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
          <h2 className="text-balance text-xl font-medium text-ink-50 sm:text-2xl">Shard Value Dashboard</h2>
          <p className="mt-2 text-sm text-ink-300">
            Visual proof of performance, participation, and cost position. Performance points are validated-only; missing
            node counts are explicitly marked as pending validation.
          </p>
          <div className="mt-4 grid gap-4 lg:grid-cols-3">
            {charts.map((chart) => (
              <article key={chart.title} className="rounded-2xl border border-ring bg-base-900 p-4">
                <h3 className="text-sm font-medium text-ink-100">{chart.title}</h3>
                <img className="mt-3 w-full rounded-lg border border-ring" src={chart.src} alt={chart.alt} loading="lazy" />
              </article>
            ))}
          </div>
          <div className="mt-4">
            <a
              href="/value-dashboard/shard-value-summary.pdf"
              className="inline-flex min-h-10 items-center rounded-lg border border-ring px-4 py-2 text-sm font-medium text-ink-100 hover:bg-base-800"
              target="_blank"
              rel="noreferrer"
            >
              Open One-Page Value Summary (PDF)
            </a>
          </div>
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
