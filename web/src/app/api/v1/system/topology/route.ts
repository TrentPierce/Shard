export const runtime = 'edge';
import { NextResponse } from "next/server"
import { fetchWithBackendFailover, shardBackendUrls } from "@/lib/server/shard-backend"
import { deriveHealthState, healthStateToLegacyStatus } from "@/lib/server/health-state"

export const dynamic = "force-dynamic"

type TopologySnapshot = {
  payload: Record<string, unknown>
  updated_at_ms: number
}

const MAX_STALE_TOPOLOGY_MS = 2 * 60 * 1000
let lastTopologySnapshot: TopologySnapshot | null = null

function updateTopologySnapshot(payload: any): void {
  if (!payload || typeof payload !== "object") return
  if (
    typeof payload.shard_peer_id !== "string" &&
    !Array.isArray(payload.bootstrap_peers) &&
    !Array.isArray(payload.listen_addrs)
  ) {
    return
  }
  lastTopologySnapshot = {
    payload: payload as Record<string, unknown>,
    updated_at_ms: Date.now(),
  }
}

function readFreshTopologySnapshot(): TopologySnapshot | null {
  if (!lastTopologySnapshot) return null
  const ageMs = Date.now() - lastTopologySnapshot.updated_at_ms
  return ageMs <= MAX_STALE_TOPOLOGY_MS ? lastTopologySnapshot : null
}

export async function GET() {
  const candidates = shardBackendUrls("/v1/system/topology")
  const fallbackBootstrapPeers = String(process.env.NEXT_PUBLIC_BOOTSTRAP_PEERS ?? "")
    .split(/[,\n ]+/)
    .map((s) => s.trim())
    .filter(Boolean)

  try {
    const { response, backend, attempts } = await fetchWithBackendFailover("/v1/system/topology", {
      timeoutMs: 6_000,
      failoverOnStatuses: [500, 502, 503, 504],
    })
    const data = await response.json()
    updateTopologySnapshot(data)
    const healthState = deriveHealthState(data, response.ok)
    if (!response.ok) {
      const cached = readFreshTopologySnapshot()
      if (cached) {
        return NextResponse.json(
          {
            ...cached.payload,
            status: healthStateToLegacyStatus(healthState),
            health_state: healthState,
            stale_snapshot: true,
            stale_snapshot_age_ms: Date.now() - cached.updated_at_ms,
            backend_status: response.status,
            backend,
            backend_attempts: attempts,
          },
          { status: 200 },
        )
      }
      return NextResponse.json(
        {
          status: healthStateToLegacyStatus(healthState),
          health_state: healthState,
          source: "proxy-non-ok",
          shard_webrtc_multiaddr: null,
          shard_quic_multiaddr: null,
          shard_ws_multiaddr: null,
          listen_addrs: [],
          bootstrap_peers: fallbackBootstrapPeers,
          known_peer_count: 0,
          public_api: false,
          backend_status: response.status,
          backend,
          backend_attempts: attempts,
          ...data,
        },
        { status: 200 },
      )
    }
    return NextResponse.json(
      {
        ...data,
        status: data?.status ?? healthStateToLegacyStatus(healthState),
        health_state: healthState,
        backend,
        backend_attempts: attempts,
      },
      { status: 200 },
    )
  } catch (error) {
    const cached = readFreshTopologySnapshot()
    if (cached) {
      return NextResponse.json(
        {
          ...cached.payload,
          status: "degraded",
          health_state: "degraded",
          stale_snapshot: true,
          stale_snapshot_age_ms: Date.now() - cached.updated_at_ms,
          backend_candidates: candidates,
          details: String(error),
        },
        { status: 200 },
      )
    }
    return NextResponse.json(
      {
        status: "unavailable",
        health_state: "unavailable",
        source: "proxy-fallback",
        shard_webrtc_multiaddr: null,
        shard_quic_multiaddr: null,
        shard_ws_multiaddr: null,
        listen_addrs: [],
        bootstrap_peers: fallbackBootstrapPeers,
        known_peer_count: 0,
        public_api: false,
        error: "Failed to get topology",
        backend_candidates: candidates,
        details: String(error),
      },
      { status: 200 }
    )
  }
}

