"use client"

import Link from "next/link"
import { useDashboardTelemetry } from "@/hooks/useDashboardTelemetry"

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

export default function HomePage() {
  const telemetry = useDashboardTelemetry()
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

  return (
    <>
      <main id="main-content" className="pb-16 pt-10 sm:pt-14">
        <section className="relative overflow-hidden rounded-3xl border border-ring bg-gradient-to-br from-base-900 to-base-800 px-6 py-12 shadow-panel sm:px-10 sm:py-16">
          <div className="relative mx-auto max-w-4xl">
            <div className="mb-5 flex flex-wrap items-center gap-3">
              <p className="text-xs font-semibold uppercase tracking-[0.22em] text-accent-400">
                Live Telemetry Dashboard
              </p>
              <span
                className={`inline-flex min-h-7 items-center rounded-full border px-2.5 text-[11px] font-semibold uppercase tracking-[0.14em] ${statusClass}`}
              >
                {telemetry.statusLabel}
              </span>
            </div>
            <h1 className="text-balance text-3xl font-semibold tracking-tight text-ink-50 sm:text-5xl">
              Browser-powered distributed inference. Run models together.
            </h1>
            <p className="mt-5 max-w-2xl text-pretty text-base text-ink-300 sm:text-lg">
              Coordinate verifiers and browser scouts from one clean control surface with real-time swarm metrics.
            </p>
            <div className="mt-8 flex flex-col gap-3 sm:flex-row sm:items-center">
              <Link
                href="/chat"
                className="inline-flex min-h-11 items-center justify-center rounded-lg bg-accent-500 px-5 py-2.5 text-sm font-medium text-base-950 transition hover:bg-accent-400"
              >
                Test the Network
              </Link>
              <Link
                href="/start"
                className="inline-flex min-h-11 items-center justify-center rounded-lg border border-ring px-5 py-2.5 text-sm font-medium text-ink-100 transition hover:bg-base-800"
              >
                Join the Swarm
              </Link>
            </div>
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
