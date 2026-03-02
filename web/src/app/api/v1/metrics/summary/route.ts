
import { NextResponse } from "next/server"
import { fetchWithBackendFailover, shardBackendUrls } from "@/lib/server/shard-backend"
import { deriveHealthState, healthStateToLegacyStatus } from "@/lib/server/health-state"
import { getChatProxySliSnapshot } from "@/lib/server/proxy-chat-sli"

export const dynamic = "force-dynamic"

type MetricsSnapshot = {
  payload: Record<string, unknown>
  updated_at_ms: number
}

const MAX_STALE_METRICS_MS = 2 * 60 * 1000
let lastMetricsSnapshot: MetricsSnapshot | null = null

function updateMetricsSnapshot(payload: any): void {
  if (!payload || typeof payload !== "object") return
  if (
    typeof payload.tokens_processed_total !== "number" &&
    typeof payload.tokens_offloaded_to_scouts_total !== "number"
  ) {
    return
  }
  lastMetricsSnapshot = {
    payload: payload as Record<string, unknown>,
    updated_at_ms: Date.now(),
  }
}

function readFreshMetricsSnapshot(): MetricsSnapshot | null {
  if (!lastMetricsSnapshot) return null
  const ageMs = Date.now() - lastMetricsSnapshot.updated_at_ms
  return ageMs <= MAX_STALE_METRICS_MS ? lastMetricsSnapshot : null
}

export async function GET() {
  // The Rust daemon exposes metrics at `/metrics/summary` (no `/v1` prefix).
  // We proxy that here for the web app and dashboards.
  const candidates = shardBackendUrls("/metrics/summary")
  const chatProxySli = getChatProxySliSnapshot()

  try {
    const { response, backend, attempts } = await fetchWithBackendFailover("/metrics/summary", {
      timeoutMs: 6_000,
      failoverOnStatuses: [500, 502, 503, 504],
    })
    const data = await response.json().catch(() => ({}))
    updateMetricsSnapshot(data)
    const healthState = deriveHealthState(data, response.ok)
    if (!response.ok) {
      const cached = readFreshMetricsSnapshot()
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
            proxy_chat_sli: chatProxySli,
          },
          { status: 200 },
        )
      }
      return NextResponse.json(
        {
          status: healthStateToLegacyStatus(healthState),
          health_state: healthState,
          backend_status: response.status,
          backend,
          backend_attempts: attempts,
          proxy_chat_sli: chatProxySli,
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
        proxy_chat_sli: chatProxySli,
      },
      { status: 200 },
    )
  } catch (error) {
    const cached = readFreshMetricsSnapshot()
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
          proxy_chat_sli: chatProxySli,
        },
        { status: 200 },
      )
    }
    return NextResponse.json(
      {
        status: "unavailable",
        health_state: "unavailable",
        backend_candidates: candidates,
        error: String(error),
        proxy_chat_sli: chatProxySli,
      },
      { status: 200 },
    )
  }
}

