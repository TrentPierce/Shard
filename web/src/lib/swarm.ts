/**
 * Shard Swarm Utilities
 *
 * - Local Shard detection (double-dip prevention)
 * - Topology fetch from Python API
 * - Shard heartbeat over local Driver API
 * - P2P networking via js-libp2p (browser Scout nodes)
 */

import { apiUrl, rustUrl } from "./config"
import { getActiveEngine, generateDrafts } from "./scout-engine"
import { getScoutId, pollForWork, reportScoutClientEvent, submitDraft } from "./scout-draft"
import { canUseLocalDaemonFallback, getPreferredLocalDaemonBase, localDaemonUrl } from "./runtime"

// ─── Types ──────────────────────────────────────────────────────────────────

export type LocalShardProbe = {
    available: boolean
    endpoint: string
}

export type Topology = {
    status: string
    source?: string
    model_id?: string
    shard_peer_id?: string
    shard_webrtc_multiaddr?: string | null
    shard_quic_multiaddr?: string | null
    shard_ws_multiaddr?: string | null
    listen_addrs?: string[]
    bootstrap_peers?: string[]
    scout_count?: number
    shard_count?: number
}

export type HandshakeResult = {
    ok: boolean
    detail: string
    rttMs?: number
}

export type WorkRequest = {
    prompt_context: string
    request_id: string
    min_tokens: number
    created_at_ms?: number
    lease_id?: string
    lease_expires_at_ms?: number
}

export type WorkResult = {
    work_id: string
    lease_id?: string
    draft_text: string
    prompt_context: string
    scout_id: string
    timestamp: number
    scout_mode: "webgpu" | "wasm"
}

export type ScoutSubmissionResult = {
    success: boolean
    detail: string
}

export type WorkPollResult = {
    work: WorkRequest | null
    transientError: boolean
    detail?: string
}

function fallbackDraftFromPrompt(_prompt: string, _maxTokens: number): string {
    // No real inference available — return empty to skip draft submission.
    // The previous echo-back approach (returning tail words of the prompt)
    // always failed verification and wasted verifier compute.
    return ""
}

async function generateDraftsWithTimeout(
    prompt: string,
    maxTokens: number,
    timeoutMs: number
): Promise<{ success: boolean; text: string; error?: string }> {
    try {
        return await Promise.race([
            generateDrafts(prompt, { maxTokens }),
            new Promise<{ success: boolean; text: string; error?: string }>((resolve) =>
                setTimeout(
                    () =>
                        resolve({
                            success: false,
                            text: "",
                            error: `generation_timeout_${timeoutMs}ms`,
                        }),
                    timeoutMs
                )
            ),
        ])
    } catch (error: any) {
        return {
            success: false,
            text: "",
            error: error?.message ?? "generateDrafts_threw",
        }
    }
}

// ─── Functions ──────────────────────────────────────────────────────────────

/**
 * Probe localhost for a running native Shard exe.
 * If detected, the browser MUST disable WebGPU and route to the local
 * Shard (double-dip prevention per the agents.md spec).
 */
function isWsTransportAddr(addr: string): boolean {
    const normalized = addr.toLowerCase()
    return (
        normalized.startsWith("ws://") ||
        normalized.startsWith("wss://") ||
        normalized.includes("/ws/") ||
        normalized.endsWith("/ws") ||
        normalized.includes("/wss/") ||
        normalized.endsWith("/wss")
    )
}

function isSecureWsTransportAddr(addr: string): boolean {
    const normalized = addr.toLowerCase()
    return (
        normalized.startsWith("wss://") ||
        normalized.includes("/wss/") ||
        normalized.endsWith("/wss")
    )
}

function collectSwarmWorkerBootstrapAddrs(
    topology: Topology | null,
    knownShardAddr: string | null,
): string[] | undefined {
    const rawCandidates = [
        knownShardAddr ?? "",
        topology?.shard_ws_multiaddr ?? "",
        ...(topology?.listen_addrs ?? []),
        ...(topology?.bootstrap_peers ?? []),
    ]
        .map((addr) => String(addr || "").trim())
        .filter(Boolean)

    const isHttpsPage = typeof window !== "undefined" && window.location.protocol === "https:"
    const deduped = new Set<string>()
    for (const addr of rawCandidates) {
        if (!isWsTransportAddr(addr)) continue
        if (isHttpsPage && !isSecureWsTransportAddr(addr)) continue
        deduped.add(addr)
    }
    const output = Array.from(deduped)
    return output.length > 0 ? output : undefined
}

export async function probeLocalShard(): Promise<LocalShardProbe> {
    if (!canUseLocalDaemonFallback()) {
        return { available: false, endpoint: "" }
    }
    const preferredBase = canUseLocalDaemonFallback() ? await getPreferredLocalDaemonBase() : null
    const endpoint = preferredBase ? localDaemonUrl("/health", preferredBase) : rustUrl("/health")
    const LATENCY_THRESHOLD_MS = 2  // Same-machine detection threshold

    try {
        const startTime = performance.now()
        const res = await fetch(endpoint, { method: "GET" })
        const rttMs = performance.now() - startTime

        if (!res.ok) return { available: false, endpoint }

        const json = await res.json()
        const isHealthy = Boolean(json?.status === "ok")

        if (isHealthy && rttMs < LATENCY_THRESHOLD_MS) {
            console.log(
                `[Double-Dip Guard] Local Shard detected at ${endpoint} ` +
                `(RTT: ${rttMs.toFixed(2)}ms < ${LATENCY_THRESHOLD_MS}ms threshold). ` +
                `Disabling WebGPU to prevent VRAM conflicts.`
            )
            return { available: true, endpoint }
        }

        return { available: isHealthy, endpoint }
    } catch {
        return { available: false, endpoint }
    }
}

/**
 * Fetch network topology.
 */
export async function fetchTopology(): Promise<Topology> {
    const degraded: Topology = { status: "degraded", shard_webrtc_multiaddr: null, shard_quic_multiaddr: null }
    const preferredBase = canUseLocalDaemonFallback() ? await getPreferredLocalDaemonBase() : null
    const endpoints = preferredBase
        ? [localDaemonUrl("/v1/system/topology", preferredBase), apiUrl("/v1/system/topology")]
        : canUseLocalDaemonFallback()
        ? [rustUrl("/v1/system/topology"), apiUrl("/v1/system/topology")]
        : [apiUrl("/v1/system/topology")]

    for (const endpoint of endpoints) {
        try {
            const res = await fetch(endpoint, {
                cache: "no-store",
                signal: AbortSignal.timeout(2500),
            })
            if (!res.ok) continue
            return (await res.json()) as Topology
        } catch {
            // try next endpoint
        }
    }

    return degraded
}

/**
 * Perform a PING/PONG heartbeat.
 */
export async function heartbeatShard(
    shardAddr: string
): Promise<HandshakeResult> {
    const preferredBase = canUseLocalDaemonFallback() ? await getPreferredLocalDaemonBase() : null
    const healthEndpoints = preferredBase
        ? [localDaemonUrl("/health", preferredBase), apiUrl("/health")]
        : canUseLocalDaemonFallback()
        ? [rustUrl("/health"), apiUrl("/health")]
        : [apiUrl("/health")]

    for (const endpoint of healthEndpoints) {
        try {
            const started = performance.now()
            const res = await fetch(endpoint, { method: "GET" })
            const rttMs = performance.now() - started

            if (!res.ok) {
                continue
            }

            const payload = await res.json()
            if (payload?.status === "ok") {
                return {
                    ok: true,
                    detail: `PONG via ${shardAddr}`,
                    rttMs,
                }
            }
        } catch {
            // try next endpoint
        }
    }

    try {
        const res = await fetch(apiUrl("/health"), { method: "GET" })
        return {
            ok: false,
            detail: `health check failed (${res.status}) for ${shardAddr}`,
        }
    } catch {
        return { ok: false, detail: `heartbeat failed for ${shardAddr}` }
    }
}

/**
 * Register the service worker for background coordination.
 */
export async function initSwarmWorker(
    knownShardAddr: string | null,
    hasLocalShard = false,
    topology: Topology | null = null
): Promise<ServiceWorkerRegistration | null> {
    if (typeof navigator === "undefined" || !("serviceWorker" in navigator)) {
        return null
    }

    const registration = await navigator.serviceWorker.register(
        "/swarm-worker.js"
    )
    await navigator.serviceWorker.ready

    const bootstrapAddrs = collectSwarmWorkerBootstrapAddrs(topology, knownShardAddr)

    registration.active?.postMessage({
        type: "INIT_SCOUT",
        knownShardAddr,
        hasLocalShard,
        bootstrapAddrs,
    })

    return registration
}

/**
 * Handle incoming work request from the swarm.
 */
export async function handleScoutWork(work: WorkRequest): Promise<ScoutSubmissionResult> {
    try {
        const engine = getActiveEngine()
        const scoutMode: "webgpu" | "wasm" = engine?.mode === "webgpu" ? "webgpu" : "wasm"
        let draftText = ""
        if (!engine) {
            draftText = fallbackDraftFromPrompt(work.prompt_context, work.min_tokens)
            void reportScoutClientEvent("fallback_draft_used", "engine_uninitialized")
        } else {
            try {
                const draftResult = await generateDraftsWithTimeout(work.prompt_context, work.min_tokens, 2500)
                draftText = draftResult.text
                if (!draftResult.success || !draftText?.trim()) {
                    draftText = fallbackDraftFromPrompt(work.prompt_context, work.min_tokens)
                    void reportScoutClientEvent("generate_failure", draftResult.error || "empty_draft_result")
                    void reportScoutClientEvent("fallback_draft_used", "empty_or_failed_generation")
                }
            } catch {
                draftText = fallbackDraftFromPrompt(work.prompt_context, work.min_tokens)
                void reportScoutClientEvent("generate_failure", "generateDrafts_threw")
                void reportScoutClientEvent("fallback_draft_used", "exception_in_generation")
            }
        }

        const scoutId = getScoutId()
        if (!draftText.trim()) {
            return {
                success: false,
                detail: "No usable draft produced; submission skipped",
            }
        }

        const result: WorkResult = {
            work_id: work.request_id,
            lease_id: work.lease_id,
            draft_text: draftText,
            prompt_context: work.prompt_context,
            scout_id: scoutId,
            timestamp: Date.now() / 1000,
            scout_mode: scoutMode,
        }

        void reportScoutClientEvent("submit_attempt", "handleScoutWork_before_submitDraftResult")
        const submissionResult = await submitDraftResult(result)
        void reportScoutClientEvent(
            submissionResult.success ? "submit_success" : "submit_network_error",
            `handleScoutWork_after_submitDraftResult:${submissionResult.detail || "no_detail"}`
        )

        return submissionResult
    } catch (error: any) {
        void reportScoutClientEvent("generate_failure", `handleScoutWork_failed:${error?.message ?? error}`)
        return {
            success: false,
            detail: `Scout work handling failed: ${error?.message ?? error}`,
        }
    }
}

/**
 * Submit a draft result.
 */
async function submitDraftResult(result: WorkResult): Promise<ScoutSubmissionResult> {
    try {
        void reportScoutClientEvent("submit_attempt", "handleScoutWork_pre_submit")
        const response = await submitDraft(result.work_id, result.draft_text, {
            promptContext: result.prompt_context,
            leaseId: result.lease_id,
            // Browser scouts can need several seconds for PoW + HTTP + tokenization.
            timeoutMs: 45000,
            maxRetries: 0,
            retryBackoffMs: 250,
            maxQueueDepth: 16,
        })
        if (response.ok) {
            void reportScoutClientEvent("submit_success", "handleScoutWork_submit_ok")
        } else {
            void reportScoutClientEvent("submit_network_error", response.detail || "handleScoutWork_submit_failed")
        }
        return {
            success: response.ok,
            detail: response.detail || (response.ok ? "Draft submitted successfully" : "Draft submission failed"),
        }
    } catch (error: any) {
        void reportScoutClientEvent("submit_network_error", `handleScoutWork_submit_exception:${error?.message ?? error}`)
        return {
            success: false,
            detail: `Failed to submit draft: ${error?.message ?? error}`,
        }
    }
}

/**
 * Request work from the API.
 */
export async function requestWork(): Promise<WorkPollResult> {
    try {
        const polled = await pollForWork(getScoutId(), {
            // Poll less aggressively so WAN scouts do not self-DOS the control plane.
            pollTimeoutMs: 4500,
            pollRetries: 2,
            pollRetryBackoffMs: 400,
            pollMaxRetryBackoffMs: 3500,
        })
        if (!polled.work) {
            if (polled.transient_error) {
                console.warn("Transient scout polling failure:", polled.detail)
            }
            return {
                work: null,
                transientError: Boolean(polled.transient_error),
                detail: polled.detail,
            }
        }

        // Transform backend response to local type
        return {
            work: {
                request_id: polled.work.request_id,
                prompt_context: polled.work.prompt_context ?? polled.work.prompt ?? "",
                min_tokens: polled.work.min_tokens ?? polled.work.max_tokens ?? 4,
                created_at_ms: polled.work.created_at_ms,
                lease_id: polled.work.lease_id,
                lease_expires_at_ms: polled.work.lease_expires_at_ms,
            },
            transientError: false,
        }
    } catch (error: any) {
        console.error("Failed to request work:", error)
        return {
            work: null,
            transientError: true,
            detail: error?.message ?? String(error),
        }
    }
}

/**
 * Start a Scout worker loop.
 */
export async function startScoutWorker(
    onRequest?: (work: WorkRequest) => void,
    onResult?: (result: ScoutSubmissionResult) => void
): Promise<() => void> {
    const pollIntervalMs = 900
    const maxBackoffMs = 15000
    let stopped = false
    let timer: ReturnType<typeof setTimeout> | null = null
    let inFlight = false
    let consecutiveFailures = 0
    let lastRuntimeEvent: "runtime_webgpu_ready" | "runtime_wasm_fallback" | null = null

    const reportRuntimeMode = async () => {
        const engine = getActiveEngine()
        const runtimeEvent: "runtime_webgpu_ready" | "runtime_wasm_fallback" =
            engine?.mode === "webgpu" ? "runtime_webgpu_ready" : "runtime_wasm_fallback"
        if (runtimeEvent === lastRuntimeEvent) {
            return
        }
        const detail = engine?.mode === "webgpu" ? "engine_mode_webgpu" : "engine_mode_wasm_or_unavailable"
        const reported = await reportScoutClientEvent(runtimeEvent, detail, undefined, undefined, {
            bypassMute: true,
        })
        if (reported) {
            lastRuntimeEvent = runtimeEvent
        }
    }

    const schedule = (delayMs: number) => {
        if (stopped) return
        timer = setTimeout(runOnce, delayMs)
    }

    const runOnce = async () => {
        if (stopped || inFlight) {
            schedule(pollIntervalMs)
            return
        }
        inFlight = true
        try {
            await reportRuntimeMode()
            const polled = await requestWork()
            const work = polled.work
            if (work) {
                onRequest?.(work)
                const result = await handleScoutWork(work)
                onResult?.(result)
                if (!result.success) {
                    consecutiveFailures += 1
                } else {
                    consecutiveFailures = 0
                }
            } else {
                consecutiveFailures = polled.transientError ? consecutiveFailures + 1 : 0
            }
        } finally {
            inFlight = false
            const backoff = consecutiveFailures > 0
                ? Math.min(maxBackoffMs, pollIntervalMs * Math.pow(2, consecutiveFailures))
                : pollIntervalMs
            const jitterMs = Math.floor(Math.random() * 250)
            schedule(backoff + jitterMs)
        }
    }

    void reportRuntimeMode()
    schedule(0)
    return () => {
        stopped = true
        if (timer) clearTimeout(timer)
    }
}
