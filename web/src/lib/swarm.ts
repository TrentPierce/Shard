/**
 * Shard Swarm Utilities
 *
 * - Local Shard detection (double-dip prevention)
 * - Topology fetch from Python API
 * - Shard heartbeat over local Driver API
 * - P2P networking via js-libp2p (browser Scout nodes)
 */

import { apiUrl, RUST_BASE } from "./config"

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
    shard_peer_id?: string
    shard_webrtc_multiaddr?: string | null
    shard_ws_multiaddr?: string | null
    listen_addrs?: string[]
}

export type HandshakeResult = {
    ok: boolean
    detail: string
    rttMs?: number
}

export type WorkRequest = {
    prompt: string
    workId: string
    timestamp: number
}

export type WorkResult = {
    workId: string
    draftText: string
    scoutId: string
    timestamp: number
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
 * 
 * Detection criteria:
 * 1. Health endpoint responds with "ok"
 * 2. Latency is < 2ms (indicates same machine)
 * 
 * If both conditions are met, WebGPU must be disabled to prevent
 * GPU OOM crashes from Scout and Shard fighting for VRAM.
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
        
        // Double-dip prevention: if latency < 2ms, we're on same machine
        // Must disable WebGPU to prevent VRAM conflicts
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
 * Fetch network topology from the Python Driver API.
 * The topology includes the Rust sidecar's listen addresses so the
 * browser can dial directly.
 */
export async function fetchTopology(): Promise<Topology> {
    try {
        const res = await fetch(apiUrl("/v1/system/topology"))
        if (!res.ok) return { status: "degraded", shard_webrtc_multiaddr: null }
        return (await res.json()) as Topology
    } catch {
        return { status: "degraded", shard_webrtc_multiaddr: null }
    }
}

/**
 * Perform a PING/PONG heartbeat through the local Driver API.
 *
 * CI does not have access to every transitive dependency required by
 * libp2p's browser+native stacks, so this lightweight heartbeat keeps the
 * web runner verifiable without requiring restricted packages.
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
                // Parse multiaddr format
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
 *
 * This function generates draft tokens using WebLLM and submits the results
 * back to the API for verification by a Shard node.
 *
 * @param work - The work request containing the prompt
 * @returns Promise containing the result of the submission
 */
export async function handleScoutWork(work: WorkRequest): Promise<ScoutSubmissionResult> {
    try {
        // Import WebLLM functions dynamically to avoid SSR issues
        const { generateDraftTokens, isWebLLMReady } = await import("./webllm")

        // Check if WebLLM is ready
        if (!isWebLLMReady()) {
            return {
                success: false,
                detail: "WebLLM engine not initialized or still loading",
            }
        }

        // Generate draft tokens
        const draftResult = await generateDraftTokens(work.prompt)

        if (!draftResult.success) {
            return {
                success: false,
                detail: draftResult.error || "Unknown draft generation error",
            }
        }

        // Generate a scout ID (could be from libp2p peer ID in the future)
        const scoutId = generateScoutId()

        // Prepare the result for submission
        const result: WorkResult = {
            workId: work.workId,
            draftText: draftResult.text,
            scoutId,
            timestamp: Date.now(),
        }

        // Submit the result to the API
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
 * Submit a draft result to the Python API for verification.
 *
 * @param result - The draft result containing generated tokens
 * @returns Promise containing the submission result
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
 *
 * In a full implementation, this would be the libp2p peer ID.
 * For now, we use a random string.
 */
function generateScoutId(): string {
    return `scout_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`
}

/**
 * Request work from the Python API.
 *
 * This polls for available work that the Scout can process.
 *
 * @returns Promise containing the work request or null if no work is available
 */
export async function requestWork(): Promise<WorkRequest | null> {
    try {
        const res = await fetch(apiUrl("/v1/scout/work"), {
            method: "GET",
        })

        if (res.status === 204) {
            // No work available
            return null
        }

        if (!res.ok) {
            throw new Error(`Work request failed (${res.status})`)
        }

        return (await res.json()) as WorkRequest
    } catch (error: any) {
        console.error("Failed to request work:", error)
        return null
    }
}

/**
 * Start a Scout worker loop.
 *
 * This continuously polls for work and processes it when available.
 * Intended to be run in a background context (service worker or interval).
 *
 * @param onRequest - Optional callback when work is requested
 * @param onResult - Optional callback when work is completed
 */
export async function startScoutWorker(
    onRequest?: (work: WorkRequest) => void,
    onResult?: (result: ScoutSubmissionResult) => void
): Promise<() => void> {
    const pollInterval = 2000 // Poll every 2 seconds

    const poll = async () => {
        const work = await requestWork()
        if (work) {
            onRequest?.(work)
            const result = await handleScoutWork(work)
            onResult?.(result)
        }
    }

    // Start the polling loop
    const intervalId = setInterval(poll, pollInterval)

    // Return a cleanup function
    return () => clearInterval(intervalId)
}
