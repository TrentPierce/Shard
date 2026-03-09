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

type BenchmarkRow = {
  scenario: string
  p95Ms: number
  tps: number
  errorPct: number
  verdict: string
}

const joinPaths: JoinPath[] = [
  {
    title: "Join from your browser",
    eyebrow: "Fastest path",
    description: "Best for non-technical users. Open the site in Chrome or Edge, click Join, and let the page do the rest.",
    href: "/start#browser",
    cta: "Use browser scout",
    checklist: [
      "Open the site in Chrome or Edge",
      "Click Join and keep the tab open",
      "Watch the status badge change from download to contributing",
    ],
  },
  {
    title: "Run a desktop verifier",
    eyebrow: "More impact",
    description: "Best for a spare PC. Download Shard GUI, let it fetch the model, then start the node and join the network.",
    href: "/start#desktop",
    cta: "Run desktop node",
    checklist: [
      "Download the latest Shard GUI release",
      "Wait for the model download to finish",
      "Save, restart the node, then click Start",
    ],
  },
  {
    title: "Build on the API",
    eyebrow: "For developers",
    description: "Point your app at the OpenAI-compatible endpoint and keep your existing chat-completions workflow.",
    href: "/start#api",
    cta: "Use the API",
    checklist: [
      "Install the SDK or call the REST endpoint",
      "Send standard chat-completions payloads",
      "Monitor live health and release status before production rollout",
    ],
  },
]

const benchmarkRows: BenchmarkRow[] = [
  { scenario: "3 Fly nodes, verifier-only", p95Ms: 986.605, tps: 0.55, errorPct: 0, verdict: "Current fast-node baseline on pinned Fly hardware" },
  { scenario: "3 Fly nodes, browser scouts attached", p95Ms: 965.127, tps: 0.55, errorPct: 0, verdict: "Fast nodes stay neutral because scouts back off when bypass is active" },
  { scenario: "1 slower local node, verifier-only", p95Ms: 7958.929, tps: 0.2167, errorPct: 0, verdict: "Local slow-node long-generation baseline" },
  { scenario: "1 slower local node, browser scouts active", p95Ms: 3471.707, tps: 0.2167, errorPct: 0, verdict: "Browser scouts cut tail latency on this local slow node" },
  { scenario: "EC2 public path, verifier-only", p95Ms: 1054.272, tps: 0.2333, errorPct: 0, verdict: "Live production slow-node baseline" },
  { scenario: "EC2 public path, live browser scouts", p95Ms: 1419.182, tps: 0.2333, errorPct: 0, verdict: "Scouts engage (100% acceptance, 5 draft tokens) but add overhead on this path" },
]

const takeaways = [
  "Fast Fly verifiers stay neutral with browser scouts attached because the daemon bypasses speculative waits and scouts back off instead of polling aggressively.",
  "Browser scouts can engage successfully on slower nodes with real accepted speculative work. On one local slow node, scouts cut p95 from 7.96s to 3.47s. On the live EC2 public path, scouts engaged (100% acceptance, 5 draft tokens per request) but did not beat verifier-only latency.",
  "Slower-node uplift is node- and path-dependent, not yet a universal production claim. The live shardnetwork.live benchmark page now completes the full browser scout bootstrap path. The pinned harness remains the source of truth.",
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
              <span className="text-ink-400">Spring RC</span>
            </div>
            <div className="mt-5 flex items-center gap-4">
              <Image src="/brand-mark.png" alt="Shard Network" width={92} height={92} className="h-20 w-20 rounded-[1.7rem] border border-white/12 bg-white/6 p-2.5 shadow-[0_18px_45px_rgba(9,21,64,0.38)]" priority />
              <div>
                <p className="text-sm uppercase tracking-[0.18em] text-ink-200">Simple distributed AI</p>
                <p className="text-sm text-ink-300">Browser scouts draft. Verifier nodes check. Apps use one standard API.</p>
              </div>
            </div>
            <h1 className="mt-6 max-w-3xl text-balance text-4xl font-semibold tracking-tight text-ink-50 sm:text-6xl">
              Join the network in minutes, even if you are not technical.
            </h1>
            <p className="mt-5 max-w-2xl text-base leading-7 text-ink-200 sm:text-lg">
              Shard lets everyday users lend browser or desktop compute to a shared inference mesh. The network is stable today on verifier nodes, and scouts now stay out of the way until there is enough real scout supply to help.
            </p>
            <div className="mt-8 flex flex-col gap-3 sm:flex-row">
              <Link href="/start#browser" className="inline-flex min-h-11 items-center justify-center rounded-xl bg-accent-500 px-5 py-3 text-sm font-semibold text-base-950 transition hover:bg-accent-400">
                Start in Browser
              </Link>
              <Link href="/start#desktop" className="inline-flex min-h-11 items-center justify-center rounded-xl border border-white/15 bg-white/5 px-5 py-3 text-sm font-semibold text-ink-50 transition hover:bg-white/10">
                Download Shard GUI
              </Link>
            </div>
            <div className="mt-6 grid gap-3 sm:grid-cols-3">
              <div className="rounded-2xl border border-white/10 bg-base-900 p-4">
                <p className="text-xs uppercase tracking-[0.18em] text-ink-400">Release status</p>
                <p className="mt-2 text-lg font-semibold text-amber-200">NO_GO</p>
                <p className="mt-1 text-sm text-ink-300">Short-request release stability is the current gate. Scout uplift is tracked separately.</p>
              </div>
              <div className="rounded-2xl border border-white/10 bg-base-900 p-4">
                <p className="text-xs uppercase tracking-[0.18em] text-ink-400">Best current path</p>
                <p className="mt-2 text-lg font-semibold text-ink-50">1 verifier, no scouts</p>
                <p className="mt-1 text-sm text-ink-300">Best current short-run baseline is 2 verifiers without scout dependence.</p>
              </div>
              <div className="rounded-2xl border border-white/10 bg-base-900 p-4">
                <p className="text-xs uppercase tracking-[0.18em] text-ink-400">Browser status</p>
                <p className="mt-2 text-lg font-semibold text-ink-50">
                  {probeResult?.eligible ? "Ready to contribute" : "Viewer mode"}
                </p>
                <p className="mt-1 text-sm text-ink-300">
                  {probeResult?.eligible
                    ? `${probeResult.browser} with ${probeResult.estimated_vram_mb}MB estimated VRAM`
                    : "Chrome or Edge with WebGPU gives the best contribution path."}
                </p>
              </div>
            </div>
          </div>

          <aside className="rounded-[1.75rem] border border-white/12 bg-base-900 p-5">
            <p className="text-xs uppercase tracking-[0.2em] text-ink-400">Live network snapshot</p>
            <div className="mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-1">
              <StatCard label="Active verifiers" value={dashboard.verifierNodes.toLocaleString()} detail="Healthy nodes currently visible" />
              <StatCard label="Active browser scouts" value={dashboard.scouts.toLocaleString()} detail="Tabs currently contributing or recently active" />
              <StatCard label="Tokens generated" value={dashboard.totalTokensGenerated.toLocaleString()} detail="Observed processed + offloaded tokens" />
              <StatCard label="Network speed" value={`${dashboard.tokensPerSecond.toLocaleString()} tok/s`} detail={dashboard.isLive ? "Live telemetry" : "Last known snapshot"} />
            </div>
            {errorMessage ? <p className="mt-4 text-sm text-amber-200">Telemetry note: {errorMessage}</p> : null}
          </aside>
        </div>
      </section>

      <section className="mt-12">
        <div className="flex items-end justify-between gap-4">
          <div>
            <p className="text-xs uppercase tracking-[0.22em] text-ink-400">Onboarding</p>
            <h2 className="mt-2 text-3xl font-semibold text-ink-50">Pick the path that matches your comfort level.</h2>
          </div>
          <Link href="/start" className="text-sm font-medium text-accent-300 hover:text-accent-200">
            Full quick start
          </Link>
        </div>
        <div className="mt-5 grid gap-4 lg:grid-cols-3">
          {joinPaths.map((path, index) => (
            <article key={path.title} className="rounded-[1.6rem] border border-ring bg-base-900/90 p-5 shadow-panel">
              <div className="flex items-center justify-between">
                <span className="text-xs uppercase tracking-[0.18em] text-accent-300">Step {index + 1}</span>
                <span className="rounded-full border border-ring px-2 py-1 text-[11px] uppercase tracking-[0.16em] text-ink-400">{path.eyebrow}</span>
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
              <Link href={path.href} className="mt-6 inline-flex min-h-11 items-center justify-center rounded-xl border border-accent-400/30 bg-accent-400/10 px-4 py-2.5 text-sm font-semibold text-accent-100 transition hover:border-accent-300 hover:bg-accent-400/15">
                {path.cta}
              </Link>
            </article>
          ))}
        </div>
      </section>

      <section className="mt-12 grid gap-6 lg:grid-cols-[0.95fr_1.05fr]">
        <article className="rounded-[1.6rem] border border-ring bg-base-900/90 p-6 shadow-panel">
          <p className="text-xs uppercase tracking-[0.22em] text-ink-400">Latest validated benchmark result</p>
          <h2 className="mt-2 text-3xl font-semibold text-ink-50">What works right now</h2>
          <p className="mt-3 text-sm leading-6 text-ink-300">
            These numbers reflect the latest validated tests on March 9, 2026. The Fly rows are from the pinned 3-node production-like comparison. The local rows are from the long-generation browser-scout check on slower hardware. The EC2 rows are from a live production-path comparison using shardnetwork.live.
          </p>
          <div className="mt-5 overflow-hidden rounded-2xl border border-ring">
            <table className="min-w-full divide-y divide-ring text-left text-sm">
              <thead className="bg-base-950/60 text-ink-300">
                <tr>
                  <th className="px-4 py-3 font-medium">Scenario</th>
                  <th className="px-4 py-3 font-medium">p95 ms</th>
                  <th className="px-4 py-3 font-medium">TPS</th>
                  <th className="px-4 py-3 font-medium">Errors</th>
                  <th className="px-4 py-3 font-medium">Takeaway</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-ring/70 bg-base-900/60 text-ink-100">
                {benchmarkRows.map((row) => (
                  <tr key={row.scenario}>
                    <td className="px-4 py-3 font-medium text-ink-50">{row.scenario}</td>
                    <td className="px-4 py-3">{row.p95Ms.toLocaleString(undefined, { maximumFractionDigits: 3 })}</td>
                    <td className="px-4 py-3">{row.tps.toLocaleString(undefined, { maximumFractionDigits: 2 })}</td>
                    <td className="px-4 py-3">{row.errorPct.toFixed(2)}%</td>
                    <td className="px-4 py-3 text-ink-300">{row.verdict}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <div className="mt-5 grid gap-3">
            {takeaways.map((line) => (
              <div key={line} className="rounded-2xl border border-ring bg-base-950/40 p-4 text-sm text-ink-200">
                {line}
              </div>
            ))}
          </div>
        </article>

        <article className="rounded-[1.6rem] border border-ring bg-base-900/90 p-6 shadow-panel">
          <div className="flex items-end justify-between gap-4">
            <div>
              <p className="text-xs uppercase tracking-[0.22em] text-ink-400">Live contribution map</p>
              <h2 className="mt-2 text-3xl font-semibold text-ink-50">Who is helping right now</h2>
            </div>
            <span className="rounded-full border border-ring px-3 py-1 text-xs uppercase tracking-[0.16em] text-ink-300">
              {telemetry.contributors.length} observed contributors
            </span>
          </div>
          <p className="mt-3 text-sm leading-6 text-ink-300">
            This view combines proxy health probes, queue data, latency, uptime, model identity, and token totals so you can see which verifier nodes are healthy and which contributors are actually carrying work.
          </p>
          <div className="mt-6 space-y-3">
            {contributionRows.length > 0 ? (
              contributionRows.map((row) => (
                <div key={`${row.role}-${row.id}`} className="rounded-2xl border border-ring bg-base-950/40 p-4">
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <div>
                      <p className="text-sm font-semibold text-ink-50">{row.id}</p>
                      <div className="mt-1 flex flex-wrap items-center gap-2">
                        <p className="text-xs uppercase tracking-[0.16em] text-ink-400">{row.label}</p>
                        <span className={`rounded-full border px-2 py-1 text-[11px] uppercase tracking-[0.14em] ${healthBadgeClass(row.health)}`}>
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
                      <p className="text-sm font-semibold text-accent-100">{formatCompact(row.tokensProcessed)} tokens</p>
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
                    <div className="h-full rounded-full bg-accent-500" style={{ width: `${row.width}%` }} />
                  </div>
                </div>
              ))
            ) : (
              <div className="rounded-2xl border border-dashed border-ring p-6 text-sm text-ink-300">
                Waiting for contributor telemetry. Once nodes or browser scouts report in, they will appear here automatically.
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
