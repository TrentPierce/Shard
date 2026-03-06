"use client"

import { useEffect, useMemo, useRef, useState } from "react"
import { setRuntimeApiBaseOverride } from "@/lib/config"
import { detectScoutCapability, type ScoutCapabilityResult } from "@/lib/scout-capability"
import { initScoutEngine } from "@/lib/scout-engine"
import { setContributionStatus } from "@/lib/contribution-status"
import { startScoutWorker } from "@/lib/swarm"
import type { ModelProgress } from "@/lib/webllm"

type BenchmarkState =
  | "booting"
  | "checking_capability"
  | "loading_model"
  | "starting_worker"
  | "contributing"
  | "failed"

function normalizeError(error: unknown): string {
  if (error instanceof Error) return error.message
  return String(error)
}

export default function BenchmarkScoutPage() {
  const [query, setQuery] = useState(() => ({
    backend: "",
    slot: "0",
    label: "Scout 0",
  }))
  const backend = query.backend
  const slot = query.slot
  const label = query.label
  const [state, setState] = useState<BenchmarkState>("booting")
  const [detail, setDetail] = useState("Preparing browser scout benchmark")
  const [capability, setCapability] = useState<ScoutCapabilityResult | null>(null)
  const [progress, setProgress] = useState<ModelProgress | null>(null)
  const [lastSuccessAtMs, setLastSuccessAtMs] = useState<number | null>(null)
  const stopWorkerRef = useRef<null | (() => void)>(null)
  const capabilityRef = useRef<ScoutCapabilityResult | null>(null)

  const statusLine = useMemo(() => {
    if (state === "loading_model" && progress) {
      return `${progress.text || "Loading model"} (${Math.round(progress.progress * 100)}%)`
    }
    if (lastSuccessAtMs) {
      return `${detail} | last success ${new Date(lastSuccessAtMs).toLocaleTimeString()}`
    }
    return detail
  }, [detail, lastSuccessAtMs, progress, state])

  useEffect(() => {
    if (typeof window === "undefined") return
    const searchParams = new URLSearchParams(window.location.search)
    const nextSlot = (searchParams.get("slot") || "0").trim()
    setQuery({
      backend: (searchParams.get("backend") || "").trim(),
      slot: nextSlot,
      label: (searchParams.get("label") || `Scout ${nextSlot}`).trim(),
    })
  }, [])

  useEffect(() => {
    let cancelled = false

    const boot = async () => {
      try {
        setRuntimeApiBaseOverride(backend || null)
        setState("checking_capability")
        setDetail("Checking WebGPU support")
        const nextCapability = await detectScoutCapability()
        if (cancelled) return
        setCapability(nextCapability)
        capabilityRef.current = nextCapability
        if (nextCapability.capability !== "webgpu") {
          const reason = nextCapability.reason || "WebGPU is required for benchmark scout mode"
          setState("failed")
          setDetail(reason)
          setContributionStatus("not_contributing", reason, nextCapability.capability)
          return
        }

        setState("loading_model")
        setDetail("Loading WebLLM draft model")
        await initScoutEngine((progress, text) => {
          if (cancelled) return
          setProgress({
            progress,
            text,
            timeElapsed: 0,
          })
        })
        if (cancelled) return

        setProgress(null)
        setState("starting_worker")
        setDetail("Starting real browser scout worker")
        setContributionStatus("initializing", "Starting benchmark scout worker", "webgpu")

        stopWorkerRef.current = await startScoutWorker(
          () => {
            if (cancelled) return
            setState("contributing")
            setDetail("Real browser scout active")
            setContributionStatus("contributing", "Benchmark scout worker active", "webgpu")
          },
          (result) => {
            if (cancelled) return
            if (result.success) {
              setLastSuccessAtMs(Date.now())
              setState("contributing")
              setDetail("Drafts are being submitted")
              return
            }
            setDetail(result.detail || "Scout worker reported a transient failure")
          },
        )

        if (cancelled) return
        setState("contributing")
        setDetail("Scout worker loop started")
      } catch (error) {
        if (cancelled) return
        const reason = normalizeError(error)
        setState("failed")
        setDetail(reason)
        setContributionStatus("degraded", reason, capabilityRef.current?.capability)
      }
    }

    void boot()
    return () => {
      cancelled = true
      stopWorkerRef.current?.()
      stopWorkerRef.current = null
      setRuntimeApiBaseOverride(null)
    }
  }, [backend])

  useEffect(() => {
    if (typeof window === "undefined") return
    ;(window as typeof window & { __SHARD_BENCHMARK_SCOUT__?: Record<string, unknown> }).__SHARD_BENCHMARK_SCOUT__ = {
      backend,
      slot,
      label,
      state,
      detail,
      capability: capability?.capability || null,
      progress: progress?.progress ?? null,
      lastSuccessAtMs,
    }
  }, [backend, capability?.capability, detail, label, lastSuccessAtMs, progress?.progress, slot, state])

  useEffect(() => {
    const suffix = progress ? ` progress=${Math.round(progress.progress * 100)}%` : ""
    console.info(`[benchmark-scout] state=${state} detail=${detail}${suffix}`)
  }, [detail, progress, state])

  return (
    <main
      id="main-content"
      data-benchmark-scout-root
      data-scout-state={state}
      data-scout-backend={backend}
      className="mx-auto max-w-3xl px-6 py-10"
    >
      <div className="rounded-[1.75rem] border border-ring bg-base-900/90 p-6 shadow-panel">
        <p className="text-xs uppercase tracking-[0.2em] text-ink-400">Benchmark Scout</p>
        <h1 className="mt-2 text-3xl font-semibold text-ink-50">{label}</h1>
        <p className="mt-3 text-sm leading-6 text-ink-300">
          This page runs the real browser WebGPU scout loop used by the website. It is intended for benchmark automation, not normal onboarding.
        </p>

        <div className="mt-6 grid gap-3 sm:grid-cols-2">
          <StatusCard label="State" value={state} />
          <StatusCard label="Backend" value={backend || "relative /api proxy"} />
          <StatusCard label="Capability" value={capability?.capability || "pending"} />
          <StatusCard label="Last success" value={lastSuccessAtMs ? new Date(lastSuccessAtMs).toLocaleTimeString() : "waiting"} />
        </div>

        <div className="mt-5 rounded-2xl border border-ring bg-base-950/40 p-4">
          <p className="text-xs uppercase tracking-[0.16em] text-ink-400">Status</p>
          <p className="mt-2 text-sm text-ink-100">{statusLine}</p>
          {progress ? (
            <div className="mt-4">
              <div className="h-3 overflow-hidden rounded-full bg-white/6">
                <div
                  className="h-full rounded-full bg-accent-500"
                  style={{ width: `${Math.max(4, Math.min(100, Math.round(progress.progress * 100)))}%` }}
                />
              </div>
            </div>
          ) : null}
        </div>
      </div>
    </main>
  )
}

function StatusCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-2xl border border-ring bg-base-950/40 p-4">
      <p className="text-xs uppercase tracking-[0.16em] text-ink-400">{label}</p>
      <p className="mt-2 text-sm font-medium text-ink-50">{value}</p>
    </div>
  )
}
