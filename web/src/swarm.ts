import { createLibp2p } from "libp2p"
import { webRTC } from "@libp2p/webrtc"
import { noise } from "@chainsafe/libp2p-noise"
import { multiaddr } from "@multiformats/multiaddr"

export type LocalOracleProbe = {
  available: boolean
  endpoint: string
}

export type Topology = {
  status: string
  oracle_webrtc_multiaddr: string | null
  source?: string
}

export type HandshakeResult = {
  ok: boolean
  detail: string
  rttMs?: number
}

export async function probeLocalOracle(): Promise<LocalOracleProbe> {
  const endpoint = "http://127.0.0.1:8080/health"
  try {
    const res = await fetch(endpoint, { method: "GET" })
    if (!res.ok) {
      return { available: false, endpoint }
    }
    const json = await res.json()
    return { available: Boolean(json?.status === "ok"), endpoint }
  } catch {
    return { available: false, endpoint }
  }
}

export async function fetchTopology(): Promise<Topology> {
  const res = await fetch("http://127.0.0.1:8000/v1/system/topology")
  if (!res.ok) {
    return { status: "degraded", oracle_webrtc_multiaddr: null }
  }
  return (await res.json()) as Topology
}

export async function initSwarmWorker(knownOracleAddr: string | null, hasLocalOracle = false): Promise<ServiceWorkerRegistration | null> {
  if (!("serviceWorker" in navigator)) {
    return null
  }

  const registration = await navigator.serviceWorker.register("/swarm-worker.js")
  await navigator.serviceWorker.ready

  registration.active?.postMessage({
    type: "INIT_SCOUT",
    knownOracleAddr,
    hasLocalOracle,
  })

  return registration
}

export async function heartbeatOracle(oracleAddr: string): Promise<HandshakeResult> {
  const node = await createLibp2p({
    transports: [webRTC()],
    connectionEncrypters: [noise()],
  })

  await node.start()
  try {
    const started = performance.now()
    const stream = await node.dialProtocol(multiaddr(oracleAddr), "/shard/1.0.0/handshake")

    const payload = JSON.stringify({ kind: "PING", sent_at_ms: Date.now() })
    const encoded = new TextEncoder().encode(payload)

    await stream.sink((async function* () {
      yield encoded
    })())

    let responseRaw = ""
    for await (const chunk of stream.source) {
      responseRaw += new TextDecoder().decode(chunk.subarray())
      if (responseRaw.includes("PONG")) {
        break
      }
    }

    const rttMs = performance.now() - started
    return { ok: responseRaw.includes("PONG"), detail: responseRaw || "no response", rttMs }
  } catch (err: any) {
    return { ok: false, detail: String(err?.message ?? err) }
  } finally {
    await node.stop()
  }
}
