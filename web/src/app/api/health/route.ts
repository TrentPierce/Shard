import { NextResponse } from "next/server"
import { fetchWithBackendFailover, shardBackendUrls } from "@/lib/server/shard-backend"
import { SHARD_VERSION } from "@/lib/version"

export const dynamic = "force-dynamic"

type ActivitySnapshot = {
  active_scouts: number
  active_browser_sessions: number
  connected_peers: number
  shard_count: number
  updated_at_ms: number
}

const MAX_STALE_ACTIVITY_MS = 2 * 60 * 1000
let lastActivitySnapshot: ActivitySnapshot | null = null

function toNonNegativeNumber(value: unknown): number | null {
  if (typeof value !== "number" || !Number.isFinite(value)) return null
  return Math.max(0, value)
}

function updateActivitySnapshot(payload: any): void {
  const nextActiveScouts = toNonNegativeNumber(payload?.active_scouts)
  const nextSessions = toNonNegativeNumber(payload?.active_browser_sessions)
  const nextPeers = toNonNegativeNumber(payload?.connected_peers)
  const nextShardCount = toNonNegativeNumber(payload?.shard_count)
  const hasAny =
    nextActiveScouts !== null ||
    nextSessions !== null ||
    nextPeers !== null ||
    nextShardCount !== null
  if (!hasAny) return

  const previous = lastActivitySnapshot
  lastActivitySnapshot = {
    active_scouts: nextActiveScouts ?? previous?.active_scouts ?? 0,
    active_browser_sessions: nextSessions ?? previous?.active_browser_sessions ?? 0,
    connected_peers: nextPeers ?? previous?.connected_peers ?? 0,
    shard_count: nextShardCount ?? previous?.shard_count ?? 0,
    updated_at_ms: Date.now(),
  }
}

function readFreshActivitySnapshot(): ActivitySnapshot | null {
  if (!lastActivitySnapshot) return null
  const ageMs = Date.now() - lastActivitySnapshot.updated_at_ms
  return ageMs <= MAX_STALE_ACTIVITY_MS ? lastActivitySnapshot : null
}

export async function GET() {
  const candidates = shardBackendUrls("/health")

  try {
    const started = performance.now()
    const { response, backend, attempts } = await fetchWithBackendFailover("/health", {
      timeoutMs: 2_500,
      totalTimeoutMs: 3_800,
      maxAttempts: 2,
      retryJitterMs: 180,
      failoverOnStatuses: [500, 502, 503, 504],
    })
    const latencyMs = Math.round(performance.now() - started)
    const data = await response.json().catch(() => ({}))
    updateActivitySnapshot(data)
    const cached = readFreshActivitySnapshot()

    if (!response.ok) {
      return NextResponse.json(
        {
          status: "degraded",
          readiness_reason: "backend_non_ok",
          ready_for_inference: false,
          active_browser_sessions: cached?.active_browser_sessions ?? 0,
          active_scouts: cached?.active_scouts ?? 0,
          connected_peers: cached?.connected_peers ?? 0,
          shard_count: cached?.shard_count ?? 0,
          stale_activity: Boolean(cached),
          stale_activity_age_ms: cached ? Date.now() - cached.updated_at_ms : undefined,
          backend_status: response.status,
          backend,
          backend_attempts: attempts,
          latency_ms: latencyMs,
          web_version: SHARD_VERSION,
          ...data,
        },
        { status: 200 },
      )
    }

    return NextResponse.json(
      { ...data, backend, backend_attempts: attempts, latency_ms: latencyMs, web_version: SHARD_VERSION },
      { status: 200 },
    )
  } catch (error) {
    const cached = readFreshActivitySnapshot()
    return NextResponse.json(
      {
        status: "degraded",
        readiness_reason: "backend_unreachable",
        ready_for_inference: false,
        active_browser_sessions: cached?.active_browser_sessions ?? 0,
        active_scouts: cached?.active_scouts ?? 0,
        connected_peers: cached?.connected_peers ?? 0,
        shard_count: cached?.shard_count ?? 0,
        stale_activity: Boolean(cached),
        stale_activity_age_ms: cached ? Date.now() - cached.updated_at_ms : undefined,
        error: "Failed to connect to backend",
        backend_candidates: candidates,
        details: String(error),
        web_version: SHARD_VERSION,
      },
      { status: 200 }
    )
  }
}
