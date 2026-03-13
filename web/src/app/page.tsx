"use client"

import Image from "next/image"
import Link from "next/link"
import { useEffect, useMemo, useState } from "react"
import { useDashboardTelemetry } from "@/hooks/useDashboardTelemetry"
import { useSwarmTelemetry } from "@/hooks/useSwarmTelemetry"
import { apiUrl } from "@/lib/config"
import { probeWebGPU, type WebGPUProbeResult } from "@/lib/webgpu-probe"

type JoinPath = {
  title: string
  eyebrow: string
  description: string
  href: string
  cta: string
  checklist: string[]
}

type Differentiator = {
  title: string
  summary: string
  detail: string
}

const joinPaths: JoinPath[] = [
  {
    title: "Run the provenance demo",
    eyebrow: "Flagship flow",
    description:
      "Submit the `research_brief` workflow and inspect the receipt chain, fallback events, and provenance graph that explain exactly where each step ran.",
    href: "/provenance",
    cta: "Open provenance demo",
    checklist: [
      "Paste a research question and source bundle",
      "Choose supply tiers, trust floor, and public-spend guardrails",
      "Inspect the execution graph and raw receipts after the run completes",
    ],
  },
  {
    title: "Bring your own capacity",
    eyebrow: "Personal or private",
    description:
      "Run Shard on your own machine or team hardware so agents can prefer personal and private capacity before they ever reach public supply.",
    href: "/start#desktop",
    cta: "Set up a node",
    checklist: [
      "Download the latest Shard GUI release",
      "Warm the local model and verify health",
      "Use it as personal or private execution capacity before public fallback",
    ],
  },
  {
    title: "Integrate the API",
    eyebrow: "Developer path",
    description:
      "Keep `/v1/chat/completions` for compatibility, then adopt the task and provenance APIs when you need workflow-level routing evidence.",
    href: "/start#api",
    cta: "Read the integration path",
    checklist: [
      "Start with the Python SDK or REST endpoints",
      "Use chat as the baseline surface and tasks for `research_brief` workflows",
      "Fetch receipts and provenance graphs to debug routing and fallback behavior",
    ],
  },
]

const differentiators: Differentiator[] = [
  {
    title: "Receipts before rhetoric",
    summary:
      "Every workflow step returns developer-facing receipts instead of opaque scheduler logs.",
    detail:
      "You can inspect candidate rankings, selected node metadata, trust tier, latency, cost, and fallback reasons as first-class artifacts.",
  },
  {
    title: "Policy-aware routing",
    summary:
      "Tasks can constrain personal, private, and public capacity with explicit trust and budget rules.",
    detail:
      "The first v1 workflow, `research_brief`, uses those policies to decide where planning, summarization, and synthesis should run.",
  },
  {
    title: "Graceful degradation",
    summary:
      "Shard makes failure behavior visible instead of pretending the happy path is the only path.",
    detail:
      "Timeouts, incompatible candidates, public fallback blocks, and restart-recovery orphaning all remain visible in the provenance graph.",
  },
  {
    title: "Compatibility without lock-in",
    summary:
      "Chat remains available as the familiar baseline while the workflow APIs carry the differentiating product surface.",
    detail:
      "Teams can start with `/v1/chat/completions`, then adopt tasks, receipts, and provenance when they need traceable routing decisions.",
  },
]

function formatCompact(value: number) {
  return value.toLocaleString(undefined, { maximumFractionDigits: value >= 100 ? 0 : 1 })
}

function formatSeconds(seconds: number | null) {
  if (!seconds || seconds <= 0) return "fresh"
  if (seconds >= 3600) return `${Math.round(seconds / 3600)}h uptime`
  if (seconds >= 60) return `${Math.round(seconds / 60)}m uptime`
  return `${seconds}s uptime`
}

function healthBadgeClass(health: string) {
  if (health === "healthy") return "border-emerald-400/30 bg-emerald-400/10 text-emerald-100"
  if (health === "degraded") return "border-amber-300/30 bg-amber-300/10 text-amber-100"
  return "border-white/10 bg-white/5 text-ink-300"
}

export default function HomePage() {
  const dashboard = useDashboardTelemetry()
  const { telemetry, errorMessage } = useSwarmTelemetry()
  const [probeResult, setProbeResult] = useState<WebGPUProbeResult | null>(null)

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
        if (cancelled) return
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

    void runProbe()
    return () => {
      cancelled = true
    }
  }, [])

  const contributionRows = useMemo(() => {
    const rows = telemetry.contributors.slice(0, 6)
    const totalTokens = rows.reduce((sum, row) => sum + Math.max(0, row.tokensProcessed), 0)
    return rows.map((row) => {
      const share = totalTokens > 0 ? (row.tokensProcessed / totalTokens) * 100 : row.efficiency
      return {
        ...row,
        width: Math.max(12, Math.min(100, Math.round(share))),
      }
    })
  }, [telemetry.contributors])

  return (
    <main id="main-content" className="pb-16 pt-8 sm:pt-12">
      <section className="overflow-hidden rounded-[2rem] border border-white/10 bg-base-800 px-6 py-8 shadow-panel sm:px-10 sm:py-12">
        <div className="grid gap-10 lg:grid-cols-[1.25fr_0.75fr] lg:items-end">
          <div>
            <div className="inline-flex items-center gap-2 rounded-full border border-white/12 bg-white/8 px-3 py-1 text-[11px] font-semibold uppercase tracking-[0.24em] text-ink-200">
              <span>Shard Network</span>
              <span className="text-ink-400">V1 Launch Candidate</span>
            </div>
            <div className="mt-5 flex items-center gap-4">
              <Image
                src="/brand-mark.png"
                alt="Shard Network"
                width={92}
                height={92}
                className="h-20 w-20 rounded-[1.7rem] border border-white/12 bg-white/6 p-2.5 shadow-[0_18px_45px_rgba(9,21,64,0.38)]"
                priority
              />
              <div>
                <p className="text-sm uppercase tracking-[0.18em] text-ink-200">
                  Cross-topology agent workflows
                </p>
                <p className="text-sm text-ink-300">
                  Receipt-first workflow observability across personal, private, and public Shard
                  capacity.
                </p>
              </div>
            </div>
            <h1 className="mt-6 max-w-3xl text-balance text-4xl font-semibold tracking-tight text-ink-50 sm:text-6xl">
              See exactly why every agent step ran where it did.
            </h1>
            <p className="mt-5 max-w-2xl text-base leading-7 text-ink-200 sm:text-lg">
              Shard V1 is built around the `research_brief` workflow: one task submission yields a
              final brief, an append-only receipt chain, and a provenance graph that explains
              routing, fallback, latency, cost, and trust tier across personal, private, and public
              supply.
            </p>
            <div className="mt-8 flex flex-col gap-3 sm:flex-row">
              <Link
                href="/provenance"
                className="inline-flex min-h-11 items-center justify-center rounded-xl bg-accent-500 px-5 py-3 text-sm font-semibold text-base-950 transition hover:bg-accent-400"
              >
                Open Provenance Demo
              </Link>
              <Link
                href="/start"
                className="inline-flex min-h-11 items-center justify-center rounded-xl border border-white/15 bg-white/5 px-5 py-3 text-sm font-semibold text-ink-50 transition hover:bg-white/10"
              >
                View Quick Start
              </Link>
            </div>
            <div className="mt-6 grid gap-3 sm:grid-cols-3">
              <div className="rounded-2xl border border-white/10 bg-base-900 p-4">
                <p className="text-xs uppercase tracking-[0.18em] text-ink-400">Flagship workflow</p>
                <p className="mt-2 text-lg font-semibold text-cyan-100">research_brief</p>
                <p className="mt-1 text-sm text-ink-300">
                  Submit source bundles, apply routing policy, and inspect a reconstructable
                  provenance graph.
                </p>
              </div>
              <div className="rounded-2xl border border-white/10 bg-base-900 p-4">
                <p className="text-xs uppercase tracking-[0.18em] text-ink-400">
                  Compatibility layer
                </p>
                <p className="mt-2 text-lg font-semibold text-ink-50">Chat stays available</p>
                <p className="mt-1 text-sm text-ink-300">
                  `/v1/chat/completions` and the chat UI remain the baseline surface while workflow
                  observability is the new product focus.
                </p>
              </div>
              <div className="rounded-2xl border border-white/10 bg-base-900 p-4">
                <p className="text-xs uppercase tracking-[0.18em] text-ink-400">Personal tier</p>
                <p className="mt-2 text-lg font-semibold text-ink-50">
                  {probeResult?.eligible
                    ? "Ready for local execution"
                    : "Use private or public tiers"}
                </p>
                <p className="mt-1 text-sm text-ink-300">
                  {probeResult?.eligible
                    ? `${probeResult.browser} with ${probeResult.estimated_vram_mb}MB estimated VRAM. Local browser execution can still handle lightweight compatibility-chat requests.`
                    : "If the browser runtime is unavailable, Shard can still route work to private or public capacity."}
                </p>
              </div>
            </div>
          </div>

          <aside className="rounded-[1.75rem] border border-white/12 bg-base-900 p-5">
            <p className="text-xs uppercase tracking-[0.2em] text-ink-400">Live network snapshot</p>
            <div className="mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-1">
              <StatCard
                label="Active verifiers"
                value={dashboard.verifierNodes.toLocaleString()}
                detail="Healthy nodes currently visible"
              />
              <StatCard
                label="Observed contributors"
                value={telemetry.contributors.length.toLocaleString()}
                detail="Personal, private, or public capacity reporting telemetry"
              />
              <StatCard
                label="Tokens processed"
                value={dashboard.totalTokensGenerated.toLocaleString()}
                detail="Observed processed and forwarded tokens"
              />
              <StatCard
                label="Network speed"
                value={`${dashboard.tokensPerSecond.toLocaleString()} tok/s`}
                detail={dashboard.isLive ? "Live telemetry" : "Last known snapshot"}
              />
            </div>
            {errorMessage ? (
              <p className="mt-4 text-sm text-amber-200">Telemetry note: {errorMessage}</p>
            ) : null}
          </aside>
        </div>
      </section>

      <section className="mt-12">
        <div className="flex items-end justify-between gap-4">
          <div>
            <p className="text-xs uppercase tracking-[0.22em] text-ink-400">Start here</p>
            <h2 className="mt-2 text-3xl font-semibold text-ink-50">
              Pick the surface that matches what you need from Shard.
            </h2>
          </div>
          <Link href="/start" className="text-sm font-medium text-accent-300 hover:text-accent-200">
            Full quick start
          </Link>
        </div>
        <div className="mt-5 grid gap-4 lg:grid-cols-3">
          {joinPaths.map((path, index) => (
            <article
              key={path.title}
              className="rounded-[1.6rem] border border-ring bg-base-900/90 p-5 shadow-panel"
            >
              <div className="flex items-center justify-between">
                <span className="text-xs uppercase tracking-[0.18em] text-accent-300">
                  Step {index + 1}
                </span>
                <span className="rounded-full border border-ring px-2 py-1 text-[11px] uppercase tracking-[0.16em] text-ink-400">
                  {path.eyebrow}
                </span>
              </div>
              <h3 className="mt-4 text-2xl font-semibold text-ink-50">{path.title}</h3>
              <p className="mt-3 text-sm leading-6 text-ink-300">{path.description}</p>
              <ul className="mt-5 space-y-2 text-sm text-ink-200">
                {path.checklist.map((item) => (
                  <li key={item} className="flex gap-3">
                    <span className="mt-1 h-2.5 w-2.5 rounded-full bg-accent-400" />
                    <span>{item}</span>
                  </li>
                ))}
              </ul>
              <Link
                href={path.href}
                className="mt-6 inline-flex min-h-11 items-center justify-center rounded-xl border border-accent-400/30 bg-accent-400/10 px-4 py-2.5 text-sm font-semibold text-accent-100 transition hover:border-accent-300 hover:bg-accent-400/15"
              >
                {path.cta}
              </Link>
            </article>
          ))}
        </div>
      </section>

      <section className="mt-12 grid gap-6 lg:grid-cols-[0.95fr_1.05fr]">
        <article className="rounded-[1.6rem] border border-ring bg-base-900/90 p-6 shadow-panel">
          <p className="text-xs uppercase tracking-[0.22em] text-ink-400">Product claims</p>
          <h2 className="mt-2 text-3xl font-semibold text-ink-50">What Shard V1 is proving</h2>
          <p className="mt-3 text-sm leading-6 text-ink-300">
            The current website is centered on workflow observability rather than generic benchmark
            claims. Shard’s first differentiated surface is the ability to show why a workflow step
            ran on personal, private, or public capacity and what happened when the ideal path
            failed.
          </p>
          <div className="mt-5 grid gap-3">
            {differentiators.map((item) => (
              <div
                key={item.title}
                className="rounded-2xl border border-ring bg-base-950/40 p-4 text-sm text-ink-200"
              >
                <p className="font-semibold text-ink-50">{item.title}</p>
                <p className="mt-2 text-ink-200">{item.summary}</p>
                <p className="mt-2 text-ink-400">{item.detail}</p>
              </div>
            ))}
          </div>
        </article>

        <article className="rounded-[1.6rem] border border-ring bg-base-900/90 p-6 shadow-panel">
          <div className="flex items-end justify-between gap-4">
            <div>
              <p className="text-xs uppercase tracking-[0.22em] text-ink-400">
                Live capacity map
              </p>
              <h2 className="mt-2 text-3xl font-semibold text-ink-50">
                Which Shard capacity is visible right now
              </h2>
            </div>
            <span className="rounded-full border border-ring px-3 py-1 text-xs uppercase tracking-[0.16em] text-ink-300">
              {telemetry.contributors.length} observed contributors
            </span>
          </div>
          <p className="mt-3 text-sm leading-6 text-ink-300">
            This view combines proxy health probes, queue data, latency, uptime, model identity,
            and token totals so you can see which contributors are healthy enough to participate in
            personal, private, or public routing.
          </p>
          <div className="mt-6 space-y-3">
            {contributionRows.length > 0 ? (
              contributionRows.map((row) => (
                <div
                  key={`${row.role}-${row.id}`}
                  className="rounded-2xl border border-ring bg-base-950/40 p-4"
                >
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <div>
                      <p className="text-sm font-semibold text-ink-50">{row.id}</p>
                      <div className="mt-1 flex flex-wrap items-center gap-2">
                        <p className="text-xs uppercase tracking-[0.16em] text-ink-400">
                          {row.label}
                        </p>
                        <span
                          className={`rounded-full border px-2 py-1 text-[11px] uppercase tracking-[0.14em] ${healthBadgeClass(row.health)}`}
                        >
                          {row.health}
                        </span>
                        {row.backend ? (
                          <span className="rounded-full border border-white/10 bg-white/5 px-2 py-1 text-[11px] text-ink-300">
                            {row.backend.replace(/^https?:\/\//, "")}
                          </span>
                        ) : null}
                      </div>
                    </div>
                    <div className="text-right">
                      <p className="text-sm font-semibold text-accent-100">
                        {formatCompact(row.tokensProcessed)} tokens
                      </p>
                      <p className="text-xs text-ink-400">efficiency {row.efficiency}%</p>
                    </div>
                  </div>
                  <div className="mt-3 flex flex-wrap gap-2 text-xs text-ink-300">
                    <span className="rounded-full border border-white/10 bg-white/5 px-2 py-1">
                      queue {row.queueDepth}
                    </span>
                    <span className="rounded-full border border-white/10 bg-white/5 px-2 py-1">
                      {row.latencyMs > 0 ? `${row.latencyMs} ms` : "latency n/a"}
                    </span>
                    <span className="rounded-full border border-white/10 bg-white/5 px-2 py-1">
                      {formatSeconds(row.uptimeSeconds)}
                    </span>
                    {row.modelId ? (
                      <span className="rounded-full border border-white/10 bg-white/5 px-2 py-1">
                        {row.modelId}
                      </span>
                    ) : null}
                    {row.rustVersion ? (
                      <span className="rounded-full border border-white/10 bg-white/5 px-2 py-1">
                        rust {row.rustVersion}
                      </span>
                    ) : null}
                  </div>
                  {row.readinessReason ? (
                    <p className="mt-3 text-xs text-amber-100">readiness: {row.readinessReason}</p>
                  ) : null}
                  <div className="mt-3 h-3 overflow-hidden rounded-full bg-white/6">
                    <div
                      className="h-full rounded-full bg-accent-500"
                      style={{ width: `${row.width}%` }}
                    />
                  </div>
                </div>
              ))
            ) : (
              <div className="rounded-2xl border border-dashed border-ring p-6 text-sm text-ink-300">
                Waiting for contributor telemetry. Once nodes report in, they will appear here
                automatically.
              </div>
            )}
          </div>
        </article>
      </section>
    </main>
  )
}

function StatCard({ label, value, detail }: { label: string; value: string; detail: string }) {
  return (
    <div className="rounded-2xl border border-white/10 bg-black/15 p-4">
      <p className="text-xs uppercase tracking-[0.18em] text-ink-400">{label}</p>
      <p className="mt-2 text-2xl font-semibold text-ink-50">{value}</p>
      <p className="mt-1 text-sm text-ink-300">{detail}</p>
    </div>
  )
}
