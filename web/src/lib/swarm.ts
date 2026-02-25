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
import { getScoutId, pollForWork, submitDraft } from "./scout-draft"
import { canUseLocalDaemonFallback } from "./runtime"

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
}

export type WorkResult = {
    work_id: string
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

// ─── Functions ──────────────────────────────────────────────────────────────

/**
 * Probe localhost for a running native Shard exe.
 * If detected, the browser MUST disable WebGPU and route to the local
 * Shard (double-dip prevention per the agents.md spec).
 */
export async function probeLocalShard(): Promise<LocalShardProbe> {
    const endpoint = rustUrl("/health")
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
    const endpoints = canUseLocalDaemonFallback()
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
    const healthEndpoints = canUseLocalDaemonFallback()
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

    // Parse bootstrap addresses from topology
    let bootstrapAddrs: string[] | undefined
    if (topology?.listen_addrs) {
        bootstrapAddrs = topology.listen_addrs
            .filter(addr => addr.includes('/ws/') || addr.startsWith('ws://') || addr.startsWith('wss://'))
            .map(addr => {
                if (addr.startsWith('ws://') || addr.startsWith('wss://')) {
                    return addr
                }
                const hostMatch = addr.match(/(?:ip4|dns4)\/([^/]+)/)
                const portMatch = addr.match(/tcp\/(\d+)/)
                if (hostMatch && portMatch) {
                    return `ws://${hostMatch[1]}:${portMatch[1]}`
                }
                return null
            })
            .filter((addr): addr is string => addr !== null)
    }

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
        
        if (!engine) {
            return {
                success: false,
                detail: "Scout engine not initialized",
            }
        }

        const draftResult = await generateDrafts(work.prompt_context, { maxTokens: work.min_tokens })

        if (!draftResult.success) {
            return {
                success: false,
                detail: draftResult.error || "Unknown draft generation error",
            }
        }

        const scoutId = getScoutId()

        const result: WorkResult = {
            work_id: work.request_id,
            draft_text: draftResult.text,
            prompt_context: work.prompt_context,
            scout_id: scoutId,
            timestamp: Date.now() / 1000,
            scout_mode: engine.mode,
        }

        const submissionResult = await submitDraftResult(result)

        return submissionResult
    } catch (error: any) {
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
        const response = await submitDraft(result.work_id, result.draft_text, {
            promptContext: result.prompt_context,
            timeoutMs: 1000,
            maxRetries: 2,
            retryBackoffMs: 250,
            maxQueueDepth: 16,
        })
        return {
            success: response.ok,
            detail: response.detail || (response.ok ? "Draft submitted successfully" : "Draft submission failed"),
        }
    } catch (error: any) {
        return {
            success: false,
            detail: `Failed to submit draft: ${error?.message ?? error}`,
        }
    }
}

/**
 * Request work from the API.
 */
export async function requestWork(): Promise<WorkRequest | null> {
    try {
        const polled = await pollForWork(getScoutId(), {
            // Keep polling tight so scouts can respond inside speculative timeout windows.
            pollTimeoutMs: 900,
            pollRetries: 0,
            pollRetryBackoffMs: 150,
        })
        if (!polled.work) {
            if (polled.transient_error) {
                console.warn("Transient scout polling failure:", polled.detail)
            }
            return null
        }
        
        // Transform backend response to local type
        return {
            request_id: polled.work.request_id,
            prompt_context: polled.work.prompt_context ?? polled.work.prompt ?? "",
            min_tokens: polled.work.min_tokens ?? polled.work.max_tokens ?? 4,
            created_at_ms: polled.work.created_at_ms
        }
    } catch (error: any) {
        console.error("Failed to request work:", error)
        return null
    }
}

/**
 * Start a Scout worker loop.
 */
export async function startScoutWorker(
    onRequest?: (work: WorkRequest) => void,
    onResult?: (result: ScoutSubmissionResult) => void
): Promise<() => void> {
    const pollIntervalMs = 250
    const maxBackoffMs = 10000
    let stopped = false
    let timer: ReturnType<typeof setTimeout> | null = null
    let inFlight = false
    let consecutiveFailures = 0

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
            const work = await requestWork()
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
                consecutiveFailures = 0
            }
        } finally {
            inFlight = false
            const backoff = consecutiveFailures > 0
                ? Math.min(maxBackoffMs, pollIntervalMs * Math.pow(2, consecutiveFailures))
                : pollIntervalMs
            schedule(backoff)
        }
    }

    schedule(0)
    return () => {
        stopped = true
        if (timer) clearTimeout(timer)
    }
}
