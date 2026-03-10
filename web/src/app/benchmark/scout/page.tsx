"use client"

import { useEffect, useMemo, useRef, useState } from "react"
import {
  parseRuntimeApiEndpointSpec,
  setRuntimeApiBaseOverride,
  setRuntimeApiHeaderOverrides,
} from "@/lib/config"
import { detectScoutCapability, type ScoutCapabilityResult } from "@/lib/scout-capability"
import { generateDrafts, initScoutEngine } from "@/lib/scout-engine"
import { setContributionStatus } from "@/lib/contribution-status"
import { startScoutWorker } from "@/lib/swarm"
import { getScoutId, reportScoutClientEvent } from "@/lib/scout-draft"
import { resolveDraftModelPreset, type ModelProgress } from "@/lib/webllm"

type BenchmarkState =
  | "booting"
  | "checking_capability"
  | "loading_model"
  | "priming_engine"
  | "registering_runtime"
  | "starting_worker"
  | "ready"
  | "contributing"
  | "failed"

function normalizeError(error: unknown): string {
  if (error instanceof Error) return error.message
  return String(error)
}

function isDirectBrowserBackend(backendUrl: string): boolean {
  if (!backendUrl) return false
  try {
    const parsed = new URL(backendUrl)
    return parsed.hostname === "127.0.0.1" || parsed.hostname === "localhost"
  } catch {
    return false
  }
}

async function inferDraftModelPreset(backendUrl: string): Promise<string> {
  if (!backendUrl) return ""
  try {
    const response = await fetch(`${backendUrl.replace(/\/$/, "")}/health`, { cache: "no-store" })
    if (!response.ok) return ""
    const health = await response.json()
    const haystack = `${String(health?.bitnet_model ?? "")} ${String(health?.model_id ?? "")}`.toLowerCase()
    if (haystack.includes("qwen")) {
      return "qwen"
    }
  } catch {
    // Best-effort inference only.
  }
  return ""
}

export default function BenchmarkScoutPage() {
  const [query, setQuery] = useState(() => ({
    backend: "",
    slot: "0",
    label: "Scout 0",
    draftModel: "",
  }))
  const backend = query.backend
  const slot = query.slot
  const label = query.label
  const draftModel = query.draftModel
  const parsedBackend = useMemo(() => parseRuntimeApiEndpointSpec(backend), [backend])
  const [state, setState] = useState<BenchmarkState>("booting")
  const [detail, setDetail] = useState("Preparing browser scout benchmark")
  const [capability, setCapability] = useState<ScoutCapabilityResult | null>(null)
  const [progress, setProgress] = useState<ModelProgress | null>(null)
  const [lastSuccessAtMs, setLastSuccessAtMs] = useState<number | null>(null)
  const [runtimeRegistered, setRuntimeRegistered] = useState(false)
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
      draftModel: (searchParams.get("draft_model") || searchParams.get("draftModel") || "").trim(),
    })
  }, [])

  useEffect(() => {
    let cancelled = false

    const registerRuntimeMode = async (attempts: number, delayMs: number): Promise<boolean> => {
      const scoutId = getScoutId()
      const directApiBase = isDirectBrowserBackend(parsedBackend.backendUrl)
        ? parsedBackend.backendUrl
        : undefined
      for (let attempt = 0; attempt < attempts; attempt += 1) {
        const ok = await reportScoutClientEvent(
          "runtime_webgpu_ready",
          "benchmark_runtime_registration",
          undefined,
          scoutId,
          { bypassMute: true, apiBase: directApiBase },
        )
        if (ok) {
          return true
        }
        if (attempt < attempts - 1) {
          await new Promise((resolve) => setTimeout(resolve, delayMs))
        }
      }
      return false
    }

    const boot = async () => {
      try {
        const directBrowserBackend = isDirectBrowserBackend(parsedBackend.backendUrl)
        const runtimeHeaders = parsedBackend.backendUrl && !directBrowserBackend
          ? {
              "x-shard-backend-url": parsedBackend.backendUrl,
              ...parsedBackend.headers,
            }
          : {}
        setRuntimeApiBaseOverride(directBrowserBackend ? parsedBackend.backendUrl : null)
        setRuntimeApiHeaderOverrides(runtimeHeaders)
        setRuntimeRegistered(false)
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
        const inferredPreset =
          draftModel ||
          (directBrowserBackend ? await inferDraftModelPreset(parsedBackend.backendUrl) : "")
        const resolvedDraftModel = inferredPreset
          ? resolveDraftModelPreset(inferredPreset)
          : undefined
        setDetail(
          resolvedDraftModel
            ? `Loading WebLLM draft model (${resolvedDraftModel})`
            : "Loading WebLLM draft model",
        )
        await initScoutEngine((progress, text) => {
          if (cancelled) return
          setProgress({
            progress,
            text,
            timeElapsed: 0,
          })
        }, {
          allowModelFallback: false,
          modelId: resolvedDraftModel,
        })
        if (cancelled) return

        setProgress(null)
        setState("priming_engine")
        setDetail("Priming WebLLM draft engine")
        const selfTest = await generateDrafts("hello from benchmark scout", { maxTokens: 1 })
        if (!selfTest.success) {
          throw new Error(selfTest.error || "WebLLM self-test draft generation failed")
        }
        if (cancelled) return

        setState("registering_runtime")
        setDetail("Registering scout runtime with verifier")
        const registered = await registerRuntimeMode(8, 750)
        if (!registered) {
          throw new Error("Failed to register browser scout runtime with verifier")
        }
        if (cancelled) return
        setRuntimeRegistered(true)

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
          {
            apiBase: directBrowserBackend ? parsedBackend.backendUrl : undefined,
          },
        )

        if (cancelled) return
        setState("ready")
        setDetail("Scout registered and waiting for work")
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
      setRuntimeApiHeaderOverrides(null)
    }
  }, [draftModel, parsedBackend.backendUrl, parsedBackend.headers])

  useEffect(() => {
    if (typeof window === "undefined") return
    ;(window as typeof window & { __SHARD_BENCHMARK_SCOUT__?: Record<string, unknown> }).__SHARD_BENCHMARK_SCOUT__ = {
      backend,
      resolvedBackend: parsedBackend.backendUrl,
      slot,
      label,
      draftModel,
      state,
      detail,
      capability: capability?.capability || null,
      progress: progress?.progress ?? null,
      lastSuccessAtMs,
      runtimeRegistered,
    }
  }, [backend, capability?.capability, detail, draftModel, label, lastSuccessAtMs, parsedBackend.backendUrl, progress?.progress, runtimeRegistered, slot, state])

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
      data-runtime-registered={runtimeRegistered ? "true" : "false"}
      data-last-submit-success-ms={lastSuccessAtMs ? String(lastSuccessAtMs) : ""}
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
          <StatusCard label="Backend" value={parsedBackend.backendUrl || "relative /api proxy"} />
          <StatusCard label="Capability" value={capability?.capability || "pending"} />
          <StatusCard label="Runtime" value={runtimeRegistered ? "registered" : "pending"} />
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
