"use client"

import { FormEvent, useEffect, useMemo, useState, useTransition } from "react"
import {
  type AgentTaskResponse,
  type CapabilityDescriptor,
  type ExecutionPolicy,
  type ExecutionReceipt,
  type ProvenanceNode,
  type ResearchSourceInput,
  type SupplyTier,
  type TrustTier,
  defaultExecutionPolicy,
  fetchCapabilities,
  fetchExecutionBundle,
  submitResearchBriefTask,
} from "@/lib/agents"

type SourceDraft = ResearchSourceInput

const trustTierOptions: { value: TrustTier; label: string }[] = [
  { value: "local", label: "Local minimum" },
  { value: "verified_mesh", label: "Verified mesh" },
  { value: "private_attested", label: "Private attested" },
  { value: "public_specialist", label: "Public specialist" },
]

const supplyTierOptions: SupplyTier[] = ["personal", "private", "public"]

const starterSources: SourceDraft[] = [
  {
    id: "market-brief",
    title: "Quarterly Market Brief",
    content:
      "Teams are routing routine work to cheaper local or private models and saving premium specialist capacity for synthesis or high-risk decisions.",
  },
  {
    id: "ops-retro",
    title: "Operations Retrospective",
    content:
      "The biggest overruns came from hidden retries and unexplained fallback to expensive providers. Engineers asked for traceable routing receipts instead of opaque logs.",
  },
]

const policyPresets: Array<{
  id: string
  label: string
  summary: string
  build: () => ExecutionPolicy
}> = [
  {
    id: "balanced",
    label: "Balanced demo",
    summary: "Good first run. Allows all three supply tiers with a modest public budget.",
    build: () => defaultExecutionPolicy(),
  },
  {
    id: "owned",
    label: "Stay on my machines",
    summary: "Keeps the workflow on personal and private capacity only.",
    build: () => ({
      ...defaultExecutionPolicy(),
      allowed_supply_tiers: ["personal", "private"],
      fallback_order: ["personal", "private"],
      max_public_spend: 0,
    }),
  },
  {
    id: "specialist",
    label: "Use specialists if needed",
    summary: "Lets Shard reach public specialist capacity for the final synthesis step.",
    build: () => ({
      ...defaultExecutionPolicy(),
      allowed_supply_tiers: ["personal", "private", "public"],
      fallback_order: ["personal", "private", "public"],
      trust_tier: "public_specialist",
      budget_limit: 2,
      max_public_spend: 0.75,
    }),
  },
] as const

function formatUsd(value?: number | null) {
  if (value == null || Number.isNaN(value)) return "n/a"
  return new Intl.NumberFormat(undefined, {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: value >= 1 ? 2 : 4,
  }).format(value)
}

function formatLatency(value?: number | null) {
  if (value == null || Number.isNaN(value)) return "n/a"
  return value >= 1000 ? `${(value / 1000).toFixed(2)}s` : `${Math.round(value)}ms`
}

function formatTime(value: number) {
  return new Date(value).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  })
}

function supplyTone(tier?: SupplyTier | null) {
  if (tier === "personal") return "border-sky-300/30 bg-sky-300/10 text-sky-100"
  if (tier === "private") return "border-emerald-300/30 bg-emerald-300/10 text-emerald-100"
  if (tier === "public") return "border-amber-300/30 bg-amber-300/10 text-amber-100"
  return "border-white/10 bg-white/5 text-ink-300"
}

function trustTone(tier?: TrustTier | null) {
  if (tier === "verified_mesh") return "border-cyan-300/30 bg-cyan-300/10 text-cyan-100"
  if (tier === "private_attested") return "border-emerald-300/30 bg-emerald-300/10 text-emerald-100"
  if (tier === "public_specialist") return "border-fuchsia-300/30 bg-fuchsia-300/10 text-fuchsia-100"
  return "border-slate-200/20 bg-slate-200/10 text-slate-100"
}

function eventTone(eventKind: ProvenanceNode["event_kind"]) {
  if (eventKind === "completed") return "border-emerald-300/25 bg-emerald-300/10"
  if (eventKind === "failed") return "border-rose-300/25 bg-rose-300/10"
  if (eventKind === "fallback_applied") return "border-amber-300/25 bg-amber-300/10"
  if (eventKind === "orphaned") return "border-orange-300/25 bg-orange-300/10"
  return "border-white/10 bg-white/5"
}

function mergePolicy(current: ExecutionPolicy, patch: Partial<ExecutionPolicy>): ExecutionPolicy {
  return { ...current, ...patch }
}

export default function ProvenancePage() {
  const [question, setQuestion] = useState(
    "How should an AI platform team route research and synthesis tasks to balance cost, trust, and latency?",
  )
  const [sources, setSources] = useState<SourceDraft[]>(starterSources)
  const [policy, setPolicy] = useState<ExecutionPolicy>(defaultExecutionPolicy())
  const [execution, setExecution] = useState<AgentTaskResponse | null>(null)
  const [executionIdInput, setExecutionIdInput] = useState("")
  const [capabilities, setCapabilities] = useState<CapabilityDescriptor[]>([])
  const [errorMessage, setErrorMessage] = useState<string | null>(null)
  const [submitting, setSubmitting] = useState(false)
  const [loadingExecution, setLoadingExecution] = useState(false)
  const [loadingCapabilities, setLoadingCapabilities] = useState(false)
  const [isPending, startTransition] = useTransition()

  useEffect(() => {
    if (typeof window !== "undefined") {
      const executionId = new URLSearchParams(window.location.search).get("execution_id")
      if (executionId) {
        setExecutionIdInput((current) => current || executionId)
      }
    }
  }, [])

  useEffect(() => {
    let cancelled = false
    async function load() {
      setLoadingCapabilities(true)
      try {
        const response = await fetchCapabilities()
        if (!cancelled) setCapabilities(response.capabilities)
      } catch (error) {
        if (!cancelled) {
          setErrorMessage(String((error as Error)?.message ?? error ?? "capability probe failed"))
        }
      } finally {
        if (!cancelled) setLoadingCapabilities(false)
      }
    }
    void load()
    return () => {
      cancelled = true
    }
  }, [])

  const sortedNodes = useMemo(
    () =>
      [...(execution?.provenance.nodes ?? [])].sort((left, right) =>
        left.timestamp_ms === right.timestamp_ms
          ? left.receipt_id.localeCompare(right.receipt_id)
          : left.timestamp_ms - right.timestamp_ms,
      ),
    [execution?.provenance.nodes],
  )
  const receiptsById = useMemo(
    () => new Map((execution?.receipts ?? []).map((receipt) => [receipt.receipt_id, receipt])),
    [execution?.receipts],
  )
  const observedCost = useMemo(
    () =>
      (execution?.receipts ?? []).reduce(
        (sum, receipt) => sum + (receipt.actual_cost_usd ?? receipt.estimated_cost_usd ?? 0),
        0,
      ),
    [execution?.receipts],
  )

  function updateSource(index: number, patch: Partial<SourceDraft>) {
    setSources((current) =>
      current.map((source, sourceIndex) =>
        sourceIndex === index ? { ...source, ...patch } : source,
      ),
    )
  }

  function addSource() {
    setSources((current) => [
      ...current,
      { id: `source-${current.length + 1}`, title: "", content: "" },
    ])
  }

  function toggleSupplyTier(tier: SupplyTier) {
    setPolicy((current) => {
      const nextAllowed = current.allowed_supply_tiers.includes(tier)
        ? current.allowed_supply_tiers.filter((value) => value !== tier)
        : [...current.allowed_supply_tiers, tier]
      const nextFallback = current.fallback_order.filter((value) => nextAllowed.includes(value))
      return mergePolicy(current, {
        allowed_supply_tiers: nextAllowed,
        fallback_order: nextFallback.length > 0 ? nextFallback : nextAllowed,
      })
    })
  }

  function applyPreset(builder: () => ExecutionPolicy) {
    setPolicy(builder())
  }

  function moveFallbackTier(tier: SupplyTier, direction: -1 | 1) {
    setPolicy((current) => {
      const order = [...current.fallback_order]
      const index = order.indexOf(tier)
      const nextIndex = index + direction
      if (index < 0 || nextIndex < 0 || nextIndex >= order.length) return current
      ;[order[index], order[nextIndex]] = [order[nextIndex], order[index]]
      return mergePolicy(current, { fallback_order: order })
    })
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setErrorMessage(null)
    setSubmitting(true)
    try {
      const payload = await submitResearchBriefTask({
        question: question.trim(),
        sources: sources.map((source, index) => ({
          id: source.id.trim() || `source-${index + 1}`,
          title: source.title?.trim() || undefined,
          content: source.content.trim(),
        })),
        policy,
      })
      startTransition(() => setExecution(payload))
      setExecutionIdInput(payload.execution.execution_id)
      setErrorMessage(payload.detail ?? null)
    } catch (error) {
      setErrorMessage(String((error as Error)?.message ?? error ?? "workflow failed"))
    } finally {
      setSubmitting(false)
    }
  }

  async function handleLoadExecution() {
    const executionId = executionIdInput.trim()
    if (!executionId) {
      setErrorMessage("Execution ID is required to load stored provenance.")
      return
    }
    setErrorMessage(null)
    setLoadingExecution(true)
    try {
      const payload = await fetchExecutionBundle(executionId)
      startTransition(() => setExecution(payload))
      setErrorMessage(payload.detail ?? null)
    } catch (error) {
      setErrorMessage(String((error as Error)?.message ?? error ?? "execution lookup failed"))
    } finally {
      setLoadingExecution(false)
    }
  }

  return (
    <main id="main-content" className="pb-16 pt-8 sm:pt-12">
      <section className="overflow-hidden rounded-[2rem] border border-white/12 bg-[radial-gradient(circle_at_top_left,_rgba(107,169,255,0.22),_transparent_30%),linear-gradient(140deg,_rgba(14,18,27,0.96),_rgba(19,28,34,0.95))] px-6 py-8 shadow-panel sm:px-10 sm:py-12">
        <div className="grid gap-10 lg:grid-cols-[0.95fr_1.05fr]">
          <div>
            <span className="inline-flex rounded-full border border-cyan-300/30 bg-cyan-300/10 px-3 py-1 text-[11px] font-semibold uppercase tracking-[0.24em] text-cyan-100">
              Flagship demo
            </span>
            <h1 className="mt-5 max-w-3xl text-balance text-4xl font-semibold tracking-tight text-ink-50 sm:text-6xl">
              Ask one question. Watch Shard show its work.
            </h1>
            <p className="mt-5 max-w-2xl text-base leading-7 text-ink-200 sm:text-lg">
              Ask one question, paste a few sources, and Shard will return the answer plus the
              receipt trail that explains where each step ran, what failed, and what the fallback
              cost.
            </p>
            <div className="mt-6 grid gap-3 sm:grid-cols-3">
              <div className="rounded-2xl border border-white/10 bg-base-950/45 p-4">
                <p className="text-xs uppercase tracking-[0.18em] text-ink-400">1. Submit</p>
                <p className="mt-2 text-sm text-ink-100">Add a question and a small source bundle.</p>
              </div>
              <div className="rounded-2xl border border-white/10 bg-base-950/45 p-4">
                <p className="text-xs uppercase tracking-[0.18em] text-ink-400">2. Route</p>
                <p className="mt-2 text-sm text-ink-100">Shard chooses between personal, private, and public capacity.</p>
              </div>
              <div className="rounded-2xl border border-white/10 bg-base-950/45 p-4">
                <p className="text-xs uppercase tracking-[0.18em] text-ink-400">3. Explain</p>
                <p className="mt-2 text-sm text-ink-100">Receipts and the graph explain each decision in plain language.</p>
              </div>
            </div>
            <div className="mt-5 rounded-2xl border border-white/10 bg-base-950/40 p-4 text-sm leading-7 text-ink-200">
              <span className="font-semibold text-ink-50">Plain-language translation:</span>{" "}
              provenance is just the step-by-step map of what happened during the run.
            </div>
          </div>

          <aside className="rounded-[1.75rem] border border-white/12 bg-base-950/55 p-5">
            <p className="text-xs uppercase tracking-[0.2em] text-ink-400">Available machines right now</p>
            <div className="mt-4 space-y-3">
              {capabilities.slice(0, 5).map((capability) => (
                <div key={capability.candidate_id} className="rounded-2xl border border-white/10 bg-base-900/80 p-4">
                  <div className="flex flex-wrap items-start justify-between gap-2">
                    <div>
                      <p className="text-sm font-semibold text-ink-50">{capability.display_name}</p>
                      <p className="text-xs text-ink-400">{capability.role ?? "worker"} · {capability.capability_tier ?? "unclassified"}</p>
                    </div>
                    <span className={`rounded-full border px-2 py-1 text-[11px] ${supplyTone(capability.supply_tier)}`}>
                      {capability.supply_tier}
                    </span>
                  </div>
                  <div className="mt-3 flex flex-wrap gap-2">
                    <span className={`rounded-full border px-2 py-1 text-[11px] ${trustTone(capability.trust_tier)}`}>
                      {capability.trust_tier.replaceAll("_", " ")}
                    </span>
                    <span className="rounded-full border border-white/10 bg-white/5 px-2 py-1 text-[11px] text-ink-300">
                      latency {formatLatency(capability.latency_ms)}
                    </span>
                  </div>
                </div>
              ))}
              {capabilities.length === 0 && !loadingCapabilities ? (
                <div className="rounded-2xl border border-dashed border-white/12 bg-white/[0.03] p-4 text-sm text-ink-300">
                  No capability descriptors reported yet.
                </div>
              ) : null}
            </div>
          </aside>
        </div>
      </section>

      <section className="mt-10 grid gap-6 lg:grid-cols-[0.88fr_1.12fr]">
        <form onSubmit={handleSubmit} className="rounded-[1.6rem] border border-ring bg-base-900/90 p-6 shadow-panel">
          <div className="flex items-center justify-between gap-4">
            <div>
              <p className="text-xs uppercase tracking-[0.22em] text-ink-400">Workflow input</p>
              <h2 className="mt-2 text-3xl font-semibold text-ink-50">Try the research brief demo</h2>
            </div>
            <button
              type="submit"
              disabled={submitting || isPending}
              className="inline-flex min-h-11 items-center justify-center rounded-xl bg-accent-500 px-5 py-3 text-sm font-semibold text-base-950 transition hover:bg-accent-400 disabled:cursor-not-allowed disabled:opacity-60"
            >
              {submitting || isPending ? "Running..." : "Run provenance demo"}
            </button>
          </div>

          <div className="mt-6 rounded-2xl border border-white/10 bg-base-950/50 p-4">
            <div className="flex flex-wrap items-center justify-between gap-4">
              <div>
                <p className="text-sm font-medium text-ink-100">Reload a stored execution</p>
                <p className="text-xs text-ink-400">Open an older run by execution ID without rerunning the workflow.</p>
              </div>
              <div className="flex w-full gap-3 sm:w-auto">
                <input
                  value={executionIdInput}
                  onChange={(event) => setExecutionIdInput(event.target.value)}
                  className="h-11 min-w-[15rem] rounded-xl border border-white/10 bg-base-900/80 px-3 text-sm text-ink-50 outline-none transition focus:border-accent-300"
                  placeholder="exec-..."
                />
                <button
                  type="button"
                  onClick={() => void handleLoadExecution()}
                  disabled={loadingExecution || isPending}
                  className="inline-flex min-h-11 items-center justify-center rounded-xl border border-white/12 bg-white/5 px-4 py-3 text-sm font-semibold text-ink-50 transition hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-60"
                >
                  {loadingExecution ? "Loading..." : "Load"}
                </button>
              </div>
            </div>
          </div>

          <label className="mt-6 block">
            <span className="text-sm font-medium text-ink-100">Question</span>
            <textarea
              value={question}
              onChange={(event) => setQuestion(event.target.value)}
              className="mt-2 min-h-32 w-full rounded-2xl border border-white/10 bg-base-950/60 px-4 py-3 text-sm text-ink-50 outline-none transition focus:border-accent-300"
            />
          </label>

          <div className="mt-6 space-y-4">
            <div className="flex justify-end">
              <button
                type="button"
                onClick={addSource}
                disabled={sources.length >= 6}
                className="rounded-full border border-white/12 bg-white/5 px-3 py-1.5 text-xs font-semibold uppercase tracking-[0.18em] text-ink-100 transition hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-50"
              >
                Add source
              </button>
            </div>
            {sources.map((source, index) => (
              <article key={`${source.id}-${index}`} className="rounded-2xl border border-white/10 bg-base-950/50 p-4">
                <div className="grid gap-3 sm:grid-cols-[0.42fr_0.58fr]">
                  <div>
                    <input
                      value={source.id}
                      onChange={(event) => updateSource(index, { id: event.target.value })}
                      className="h-11 w-full rounded-xl border border-white/10 bg-base-900/80 px-3 text-sm text-ink-50 outline-none transition focus:border-accent-300"
                      placeholder="source-id"
                    />
                    <input
                      value={source.title ?? ""}
                      onChange={(event) => updateSource(index, { title: event.target.value })}
                      className="mt-3 h-11 w-full rounded-xl border border-white/10 bg-base-900/80 px-3 text-sm text-ink-50 outline-none transition focus:border-accent-300"
                      placeholder="Optional title"
                    />
                  </div>
                  <textarea
                    value={source.content}
                    onChange={(event) => updateSource(index, { content: event.target.value })}
                    className="min-h-36 w-full rounded-xl border border-white/10 bg-base-900/80 px-3 py-3 text-sm text-ink-50 outline-none transition focus:border-accent-300"
                    placeholder="Paste the source content used for the research brief"
                  />
                </div>
                <div className="mt-3 flex justify-end">
                  <button
                    type="button"
                    onClick={() =>
                      setSources((current) =>
                        current.length <= 1
                          ? current
                          : current.filter((_, sourceIndex) => sourceIndex !== index),
                      )
                    }
                    disabled={sources.length <= 1}
                    className="rounded-full border border-rose-300/20 bg-rose-300/10 px-3 py-1.5 text-xs font-semibold uppercase tracking-[0.18em] text-rose-100 transition hover:bg-rose-300/15 disabled:cursor-not-allowed disabled:opacity-40"
                  >
                    Remove
                  </button>
                </div>
              </article>
            ))}
          </div>

          <div className="mt-6 rounded-2xl border border-white/10 bg-base-950/50 p-4">
            <p className="text-sm font-medium text-ink-100">Choose a starting policy</p>
            <p className="mt-1 text-xs text-ink-400">
              Start with a preset, then fine-tune the rules below if you want.
            </p>
            <div className="mt-4 grid gap-3 md:grid-cols-3">
              {policyPresets.map((preset) => (
                <button
                  key={preset.id}
                  type="button"
                  onClick={() => applyPreset(preset.build)}
                  className="rounded-2xl border border-white/10 bg-base-900/70 p-4 text-left transition hover:border-accent-300/60 hover:bg-base-900"
                >
                  <p className="text-sm font-semibold text-ink-50">{preset.label}</p>
                  <p className="mt-2 text-xs leading-6 text-ink-300">{preset.summary}</p>
                </button>
              ))}
            </div>
          </div>

          <div className="mt-6 rounded-2xl border border-white/10 bg-base-950/50 p-4">
            <div className="flex flex-wrap items-center justify-between gap-4">
              <div>
                <p className="text-sm font-medium text-ink-100">Execution policy</p>
                <p className="text-xs text-ink-400">
                  Tell Shard what it is allowed to use, how careful it should be, and how much it
                  can spend on public capacity.
                </p>
              </div>
              <select
                value={policy.trust_tier}
                onChange={(event) =>
                  setPolicy((current) =>
                    mergePolicy(current, { trust_tier: event.target.value as TrustTier }),
                  )
                }
                className="h-11 rounded-xl border border-white/10 bg-base-900/80 px-3 text-sm text-ink-50 outline-none transition focus:border-accent-300"
              >
                {trustTierOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </div>

            <div className="mt-4 flex flex-wrap gap-2">
              {supplyTierOptions.map((tier) => {
                const active = policy.allowed_supply_tiers.includes(tier)
                return (
                  <button
                    key={tier}
                    type="button"
                    onClick={() => toggleSupplyTier(tier)}
                    className={`rounded-full border px-3 py-2 text-xs font-semibold uppercase tracking-[0.18em] transition ${
                      active
                        ? supplyTone(tier)
                        : "border-white/10 bg-white/5 text-ink-300 hover:bg-white/10"
                    }`}
                  >
                    {tier}
                  </button>
                )
              })}
            </div>
            <div className="mt-4 grid gap-3 sm:grid-cols-3">
              <InfoChip
                title="Allowed supply"
                body="These buttons decide which machines Shard may use at all."
              />
              <InfoChip
                title="Fallback order"
                body="This order decides where Shard looks next if the first choice fails."
              />
              <InfoChip
                title="Public spend cap"
                body="This is the most Shard may spend on public capacity for one run."
              />
            </div>

            <div className="mt-4 grid gap-4 sm:grid-cols-3">
              <input
                type="number"
                min="0"
                step="0.01"
                value={policy.budget_limit ?? 0}
                onChange={(event) =>
                  setPolicy((current) =>
                    mergePolicy(current, { budget_limit: Number(event.target.value) || 0 }),
                  )
                }
                className="h-11 rounded-xl border border-white/10 bg-base-900/80 px-3 text-sm text-ink-50 outline-none transition focus:border-accent-300"
                placeholder="Budget limit (USD)"
              />
              <input
                type="number"
                min="1000"
                step="500"
                value={policy.deadline_ms ?? 45_000}
                onChange={(event) =>
                  setPolicy((current) =>
                    mergePolicy(current, { deadline_ms: Number(event.target.value) || 45_000 }),
                  )
                }
                className="h-11 rounded-xl border border-white/10 bg-base-900/80 px-3 text-sm text-ink-50 outline-none transition focus:border-accent-300"
                placeholder="Deadline (ms)"
              />
              <input
                type="number"
                min="0"
                step="0.01"
                value={policy.max_public_spend ?? 0}
                onChange={(event) =>
                  setPolicy((current) =>
                    mergePolicy(current, {
                      max_public_spend: Number(event.target.value) || 0,
                    }),
                  )
                }
                className="h-11 rounded-xl border border-white/10 bg-base-900/80 px-3 text-sm text-ink-50 outline-none transition focus:border-accent-300"
                placeholder="Max public spend"
              />
            </div>

            <div className="mt-4 grid gap-4 sm:grid-cols-[1fr_0.9fr]">
              <input
                type="text"
                value={policy.capability_tags.join(", ")}
                onChange={(event) =>
                  setPolicy((current) =>
                    mergePolicy(current, {
                      capability_tags: event.target.value
                        .split(",")
                        .map((tag) => tag.trim())
                        .filter(Boolean),
                    }),
                  )
                }
                className="h-11 rounded-xl border border-white/10 bg-base-900/80 px-3 text-sm text-ink-50 outline-none transition focus:border-accent-300"
                placeholder="Capability tags (comma separated)"
              />
              <input
                type="text"
                value={policy.data_residency ?? ""}
                onChange={(event) =>
                  setPolicy((current) =>
                    mergePolicy(current, { data_residency: event.target.value || null }),
                  )
                }
                className="h-11 rounded-xl border border-white/10 bg-base-900/80 px-3 text-sm text-ink-50 outline-none transition focus:border-accent-300"
                placeholder="Optional data residency rule (for example us)"
              />
            </div>

            <div className="mt-4 flex flex-wrap gap-3">
              {policy.fallback_order.map((tier, index) => (
                <div key={tier} className="flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-3 py-2">
                  <span className={`rounded-full border px-2 py-1 text-[11px] ${supplyTone(tier)}`}>
                    {index + 1}. {tier}
                  </span>
                  <button type="button" onClick={() => moveFallbackTier(tier, -1)} className="text-xs text-ink-400 hover:text-ink-100">
                    Up
                  </button>
                  <button type="button" onClick={() => moveFallbackTier(tier, 1)} className="text-xs text-ink-400 hover:text-ink-100">
                    Down
                  </button>
                </div>
              ))}
            </div>
          </div>

          {errorMessage ? (
            <div className="mt-6 rounded-2xl border border-rose-300/20 bg-rose-300/10 p-4 text-sm text-rose-100">
              {errorMessage}
            </div>
          ) : null}
        </form>

        <div className="space-y-6">
          <section className="rounded-[1.6rem] border border-ring bg-base-900/90 p-6 shadow-panel">
            <div className="flex flex-wrap items-start justify-between gap-4">
              <div>
                <p className="text-xs uppercase tracking-[0.22em] text-ink-400">Execution summary</p>
                <h2 className="mt-2 text-3xl font-semibold text-ink-50">
                  {execution ? "Workflow result" : "Run the demo to inspect the workflow trail"}
                </h2>
                <p className="mt-2 text-sm leading-6 text-ink-300">
                  This panel shows the finished answer first, then the map of how Shard got there.
                </p>
              </div>
              {execution ? (
                <span className={`rounded-full border px-3 py-2 text-xs font-semibold uppercase tracking-[0.18em] ${
                  execution.execution.status === "completed"
                    ? "border-emerald-300/25 bg-emerald-300/10 text-emerald-100"
                    : execution.execution.status === "failed"
                      ? "border-rose-300/25 bg-rose-300/10 text-rose-100"
                      : "border-amber-300/25 bg-amber-300/10 text-amber-100"
                }`}>
                  {execution.execution.status.replaceAll("_", " ")}
                </span>
              ) : null}
            </div>

            {execution ? (
              <>
                <div className="mt-5 grid gap-3 sm:grid-cols-4">
                  <StatCard label="Execution ID" value={execution.execution.execution_id} />
                  <StatCard label="Receipts" value={String(execution.receipts.length)} />
                  <StatCard label="Failures" value={String(execution.receipts.filter((receipt) => receipt.event_kind === "failed").length)} />
                  <StatCard label="Observed cost" value={formatUsd(observedCost)} />
                </div>
                <div className="mt-4 grid gap-3 sm:grid-cols-2">
                  <StatCard label="Question" value={execution.receipts.find((receipt) => receipt.task_context)?.task_context?.question ?? execution.execution.question ?? "n/a"} />
                  <StatCard label="Sources" value={String(execution.receipts.find((receipt) => receipt.task_context)?.task_context?.source_count ?? execution.execution.source_count)} />
                </div>
                <article className="mt-5 rounded-[1.4rem] border border-white/10 bg-[linear-gradient(135deg,_rgba(18,32,44,0.78),_rgba(11,18,24,0.92))] p-5">
                  <p className="text-xs uppercase tracking-[0.18em] text-ink-400">Final brief</p>
                  <p className="mt-4 whitespace-pre-wrap text-sm leading-7 text-ink-100">
                    {execution.execution.result?.brief ?? "No completed artifact yet."}
                  </p>
                </article>
              </>
            ) : (
              <div className="mt-5 rounded-[1.4rem] border border-dashed border-white/12 bg-white/[0.03] p-5 text-sm leading-6 text-ink-300">
                Run the workflow to get the answer, the receipt chain, and the graph together in one place.
              </div>
            )}
          </section>

          <section className="rounded-[1.6rem] border border-ring bg-base-900/90 p-6 shadow-panel">
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="text-xs uppercase tracking-[0.22em] text-ink-400">Step-by-step map</p>
                <h2 className="mt-2 text-3xl font-semibold text-ink-50">Why the workflow ran this way</h2>
              </div>
              {execution?.provenance.incomplete ? (
                <span className="rounded-full border border-amber-300/25 bg-amber-300/10 px-3 py-2 text-xs font-semibold uppercase tracking-[0.18em] text-amber-100">
                  Incomplete
                </span>
              ) : null}
            </div>

            <div className="mt-5 space-y-3">
              {sortedNodes.length > 0 ? (
                <div className="rounded-2xl border border-white/10 bg-base-950/45 p-4 text-sm text-ink-200">
                  Read the map from top to bottom. Green means a step finished, red means it
                  failed, and amber means Shard had to fall back to a backup route.
                </div>
              ) : null}
              {sortedNodes.map((node) => {
                const parent = node.parent_receipt_id
                  ? receiptsById.get(node.parent_receipt_id)
                  : null
                return (
                  <article key={node.receipt_id} className={`rounded-2xl border p-4 ${eventTone(node.event_kind)}`}>
                    <div className="flex flex-wrap items-start justify-between gap-3">
                      <div>
                        <p className="text-sm font-semibold text-ink-50">
                          {node.label ?? `${node.step_kind ?? node.step_id} · ${node.event_kind}`}
                        </p>
                        <p className="mt-1 text-xs text-ink-400">
                          {formatTime(node.timestamp_ms)}
                          {parent ? ` · parent ${parent.step_kind ?? parent.step_id}` : " · root"}
                        </p>
                      </div>
                      <div className="flex flex-wrap gap-2">
                        <span className={`rounded-full border px-2 py-1 text-[11px] ${supplyTone(node.supply_tier)}`}>
                          {node.supply_tier ?? "controller"}
                        </span>
                        <span className={`rounded-full border px-2 py-1 text-[11px] ${trustTone(node.trust_tier)}`}>
                          {(node.trust_tier ?? "local").replaceAll("_", " ")}
                        </span>
                      </div>
                    </div>
                    <div className="mt-3 grid gap-3 sm:grid-cols-4">
                      <StatCard label="Latency" value={formatLatency(node.latency_ms)} compact />
                      <StatCard label="Estimated" value={formatUsd(node.estimated_cost_usd)} compact />
                      <StatCard label="Observed" value={formatUsd(node.actual_cost_usd)} compact />
                      <StatCard label="Candidate" value={node.selected_candidate?.display_name ?? "controller"} compact />
                    </div>
                    {node.summary ? <p className="mt-3 text-sm leading-6 text-ink-100">{node.summary}</p> : null}
                    {node.failure_reason ? <p className="mt-2 text-sm text-rose-100">Failure: {node.failure_reason}</p> : null}
                    {node.fallback_reason ? <p className="mt-2 text-sm text-amber-100">Fallback: {node.fallback_reason}</p> : null}
                  </article>
                )
              })}

              {sortedNodes.length === 0 ? (
                <div className="rounded-2xl border border-dashed border-white/12 bg-white/[0.03] p-5 text-sm leading-6 text-ink-300">
                  Run the workflow to populate the receipt graph that Shard rebuilds from `parent_receipt_id` links.
                </div>
              ) : null}
            </div>
          </section>

          <section className="rounded-[1.6rem] border border-ring bg-base-900/90 p-6 shadow-panel">
            <p className="text-xs uppercase tracking-[0.22em] text-ink-400">Receipts</p>
            <h2 className="mt-2 text-3xl font-semibold text-ink-50">Raw receipts</h2>
            <div className="mt-5 space-y-3">
              {(execution?.receipts ?? []).map((receipt: ExecutionReceipt) => (
                <details
                  key={receipt.receipt_id}
                  className="overflow-hidden rounded-2xl border border-white/10 bg-base-950/55"
                >
                  <summary className="cursor-pointer list-none px-4 py-3">
                    <div className="flex flex-wrap items-center justify-between gap-3">
                      <div>
                        <p className="text-sm font-semibold text-ink-50">
                          {receipt.step_kind ?? receipt.step_id} · {receipt.event_kind}
                        </p>
                        <p className="mt-1 text-xs text-ink-400">
                          {receipt.selected_candidate?.display_name ?? "controller"} · {formatTime(receipt.timestamp_ms)}
                        </p>
                      </div>
                      <span className={`rounded-full border px-2 py-1 text-[11px] ${supplyTone(receipt.supply_tier)}`}>
                        {receipt.supply_tier ?? "controller"}
                      </span>
                    </div>
                  </summary>
                  <div className="border-t border-white/10 px-4 py-4">
                    <pre className="overflow-x-auto whitespace-pre-wrap text-xs leading-6 text-ink-200">
                      {JSON.stringify(receipt, null, 2)}
                    </pre>
                  </div>
                </details>
              ))}
            </div>
          </section>
        </div>
      </section>
    </main>
  )
}

function StatCard({
  label,
  value,
  compact = false,
}: {
  label: string
  value: string
  compact?: boolean
}) {
  return (
    <div className="rounded-2xl border border-white/10 bg-base-950/55 p-4">
      <p className={`${compact ? "text-[11px]" : "text-xs"} uppercase tracking-[0.18em] text-ink-400`}>
        {label}
      </p>
      <p className={`${compact ? "mt-1 text-sm" : "mt-2 text-lg"} font-semibold text-ink-50`}>
        {value}
      </p>
    </div>
  )
}

function InfoChip({ title, body }: { title: string; body: string }) {
  return (
    <div className="rounded-2xl border border-white/10 bg-base-900/70 p-3">
      <p className="text-xs uppercase tracking-[0.18em] text-ink-400">{title}</p>
      <p className="mt-2 text-xs leading-6 text-ink-200">{body}</p>
    </div>
  )
}
