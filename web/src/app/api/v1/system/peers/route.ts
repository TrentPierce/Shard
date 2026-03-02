export const runtime = 'edge';
import { NextResponse } from "next/server"
import { fetchWithBackendFailover, shardBackendUrls } from "@/lib/server/shard-backend"
import { deriveHealthState, healthStateToLegacyStatus } from "@/lib/server/health-state"

export const dynamic = "force-dynamic"

type PeersSnapshot = {
  payload: Record<string, unknown>
  updated_at_ms: number
}

const MAX_STALE_PEERS_MS = 2 * 60 * 1000
let lastPeersSnapshot: PeersSnapshot | null = null

function updatePeersSnapshot(payload: any): void {
  if (!payload || typeof payload !== "object") return
  if (!Array.isArray(payload.peers) && typeof payload.count !== "number") return
  lastPeersSnapshot = {
    payload: payload as Record<string, unknown>,
    updated_at_ms: Date.now(),
  }
}

function readFreshPeersSnapshot(): PeersSnapshot | null {
  if (!lastPeersSnapshot) return null
  const ageMs = Date.now() - lastPeersSnapshot.updated_at_ms
  return ageMs <= MAX_STALE_PEERS_MS ? lastPeersSnapshot : null
}

export async function GET() {
  const candidates = shardBackendUrls("/v1/system/peers")

  try {
    const { response, backend, attempts } = await fetchWithBackendFailover("/v1/system/peers", {
      timeoutMs: 6_000,
      failoverOnStatuses: [500, 502, 503, 504, 521, 530],
    })
    const data = await response.json()
    updatePeersSnapshot(data)
    const healthState = deriveHealthState(data, response.ok)
    if (!response.ok) {
      const cached = readFreshPeersSnapshot()
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
          peers: [],
          count: 0,
          status: healthStateToLegacyStatus(healthState),
          health_state: healthState,
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
    const cached = readFreshPeersSnapshot()
    if (cached) {
      return NextResponse.json(
        {
          ...cached.payload,
          status: "degraded",
          health_state: "degraded",
          stale_snapshot: true,
          stale_snapshot_age_ms: Date.now() - cached.updated_at_ms,
          backend_candidates: candidates,
          error: String(error),
        },
        { status: 200 },
      )
    }
    return NextResponse.json(
      {
        peers: [],
        count: 0,
        status: "unavailable",
        health_state: "unavailable",
        backend_candidates: candidates,
        error: String(error),
      },
      { status: 200 },
    )
  }
}

