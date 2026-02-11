/**
 * Shard Swarm Utilities
 *
 * - Local Oracle detection (double-dip prevention)
 * - Topology fetch from Python API
 * - libp2p heartbeat handshake
 */

import { createLibp2p } from "libp2p"
import { webRTC } from "@libp2p/webrtc"
import { noise } from "@chainsafe/libp2p-noise"
import { multiaddr } from "@multiformats/multiaddr"

// ─── Types ──────────────────────────────────────────────────────────────────

export type LocalOracleProbe = {
    available: boolean
    endpoint: string
}

export type Topology = {
    status: string
    source?: string
    oracle_peer_id?: string
    oracle_webrtc_multiaddr?: string | null
    oracle_ws_multiaddr?: string | null
    listen_addrs?: string[]
}

export type HandshakeResult = {
    ok: boolean
    detail: string
    rttMs?: number
}

// ─── Python API base ────────────────────────────────────────────────────────

const API_BASE = "http://127.0.0.1:8000"

// ─── Functions ──────────────────────────────────────────────────────────────

/**
 * Probe localhost for a running native Oracle exe.
 * If detected, the browser MUST disable WebGPU and route to the local
 * Oracle (double-dip prevention per the agents.md spec).
 */
export async function probeLocalOracle(): Promise<LocalOracleProbe> {
    const endpoint = `${API_BASE}/health`
    try {
        const res = await fetch(endpoint, { method: "GET" })
        if (!res.ok) return { available: false, endpoint }
        const json = await res.json()
        return { available: Boolean(json?.status === "ok"), endpoint }
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
        const res = await fetch(`${API_BASE}/v1/system/topology`)
        if (!res.ok) return { status: "degraded", oracle_webrtc_multiaddr: null }
        return (await res.json()) as Topology
    } catch {
        return { status: "degraded", oracle_webrtc_multiaddr: null }
    }
}

/**
 * Perform a PING/PONG heartbeat to the Oracle via libp2p request-response.
 */
export async function heartbeatOracle(
    oracleAddr: string
): Promise<HandshakeResult> {
    try {
        const node = await createLibp2p({
            transports: [webRTC()],
            connectionEncrypters: [noise()],
        })

        await node.start()
        try {
            const started = performance.now()
            const stream = await node.dialProtocol(
                multiaddr(oracleAddr),
                "/shard/1.0.0/handshake"
            )

            const payload = JSON.stringify({ kind: "PING", sent_at_ms: Date.now() })
            const encoded = new TextEncoder().encode(payload)

            await stream.sink(
                (async function* () {
                    yield encoded
                })()
            )

            let responseRaw = ""
            for await (const chunk of stream.source) {
                responseRaw += new TextDecoder().decode(chunk.subarray())
                if (responseRaw.includes("PONG")) break
            }

            const rttMs = performance.now() - started
            return {
                ok: responseRaw.includes("PONG"),
                detail: responseRaw || "no response",
                rttMs,
            }
        } catch (err: any) {
            return { ok: false, detail: String(err?.message ?? err) }
        } finally {
            await node.stop()
        }
    } catch (err: any) {
        return { ok: false, detail: `libp2p init failed: ${err?.message ?? err}` }
    }
}

/**
 * Register the service worker for background coordination.
 */
export async function initSwarmWorker(
    knownOracleAddr: string | null,
    hasLocalOracle = false
): Promise<ServiceWorkerRegistration | null> {
    if (typeof navigator === "undefined" || !("serviceWorker" in navigator)) {
        return null
    }

    const registration = await navigator.serviceWorker.register(
        "/swarm-worker.js"
    )
    await navigator.serviceWorker.ready

    registration.active?.postMessage({
        type: "INIT_SCOUT",
        knownOracleAddr,
        hasLocalOracle,
    })

    return registration
}
