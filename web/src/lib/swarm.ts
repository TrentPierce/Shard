/**
 * Shard Swarm Utilities
 *
 * - Local Shard detection (double-dip prevention)
 * - Topology fetch from Python API
 * - Shard heartbeat over local Driver API
 * - P2P networking via js-libp2p (browser Scout nodes)
 */

import { apiUrl } from "./config"
import { getActiveEngine, generateDrafts } from "./scout-engine"

// Re-export P2P functions for convenience
export {
    initP2P,
    subscribeToWork,
    subscribeToResults,
    publishResult,
    getPeerId,
    getPeerCount,
    isReady,
    stopP2P,
    type P2PConfig,
    type WorkMessage,
    type WorkResultMessage,
    type WorkHandler,
    type ResultHandler,
} from "./p2p"

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
    const endpoint = apiUrl("/health")
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
    try {
        const res = await fetch(apiUrl("/v1/system/topology"))
        if (!res.ok) return { status: "degraded", shard_webrtc_multiaddr: null, shard_quic_multiaddr: null }
        return (await res.json()) as Topology
    } catch {
        return { status: "degraded", shard_webrtc_multiaddr: null, shard_quic_multiaddr: null }
    }
}

/**
 * Perform a PING/PONG heartbeat.
 */
export async function heartbeatShard(
    shardAddr: string
): Promise<HandshakeResult> {
    try {
        const started = performance.now()
        const res = await fetch(apiUrl("/health"), { method: "GET" })
        const rttMs = performance.now() - started

        if (!res.ok) {
            return {
                ok: false,
                detail: `health check failed (${res.status}) for ${shardAddr}`,
                rttMs,
            }
        }

        const payload = await res.json()
        if (payload?.status === "ok") {
            return {
                ok: true,
                detail: `PONG via ${shardAddr}`,
                rttMs,
            }
        }

        return {
            ok: false,
            detail: `unexpected response for ${shardAddr}`,
            rttMs,
        }
    } catch (err: any) {
        return { ok: false, detail: `heartbeat failed: ${err?.message ?? err}` }
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

        const scoutId = generateScoutId()

        const result: WorkResult = {
            work_id: work.request_id,
            draft_text: draftResult.text,
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
        const res = await fetch(apiUrl("/v1/scout/draft"), {
            method: "POST",
            headers: {
                "Content-Type": "application/json",
            },
            body: JSON.stringify(result),
        })

        if (!res.ok) {
            return {
                success: false,
                detail: `API submission failed (${res.status})`,
            }
        }

        const data = await res.json()
        return {
            success: true,
            detail: data?.detail || "Draft submitted successfully",
        }
    } catch (error: any) {
        return {
            success: false,
            detail: `Failed to submit draft: ${error?.message ?? error}`,
        }
    }
}

/**
 * Generate a unique scout identifier.
 */
function generateScoutId(): string {
    if (typeof window === "undefined") {
        return `scout_${Date.now()}_${Math.random().toString(36).slice(2, 11)}`
    }

    const key = "shard-scout-id"
    const existing = localStorage.getItem(key)
    if (existing) return existing

    const created = `scout_${Date.now()}_${Math.random().toString(36).slice(2, 11)}`
    localStorage.setItem(key, created)
    return created
}

/**
 * Request work from the API.
 */
export async function requestWork(): Promise<WorkRequest | null> {
    try {
        const res = await fetch(apiUrl("/v1/scout/work"), {
            method: "GET",
        })

        if (res.status === 204 || res.status === 404) {
            return null
        }

        if (!res.ok) {
            const text = await res.text()
            if (text.includes("null")) return null
            throw new Error(`Work request failed (${res.status})`)
        }

        const data = await res.json()
        if (!data || !data.work) return null
        
        // Transform backend response to local type
        return {
            request_id: data.work.request_id,
            prompt_context: data.work.prompt_context,
            min_tokens: data.work.min_tokens,
            created_at_ms: data.work.created_at_ms
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
    const pollInterval = 2000 

    const poll = async () => {
        const work = await requestWork()
        if (work) {
            onRequest?.(work)
            const result = await handleScoutWork(work)
            onResult?.(result)
        }
    }

    const intervalId = setInterval(poll, pollInterval)
    return () => clearInterval(intervalId)
}
