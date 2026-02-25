import { NextResponse } from "next/server"
import { fetchWithBackendFailover, shardBackendUrls } from "@/lib/server/shard-backend"
import { SHARD_VERSION } from "@/lib/version"

export const dynamic = "force-dynamic"

export async function GET() {
  const candidates = shardBackendUrls("/health")

  try {
    const started = performance.now()
    const { response, backend, attempts } = await fetchWithBackendFailover("/health", {
      timeoutMs: 30_000,
      failoverOnStatuses: [500, 502, 503, 504],
    })
    const latencyMs = Math.round(performance.now() - started)
    const data = await response.json()
    return NextResponse.json(
      { ...data, backend, backend_attempts: attempts, latency_ms: latencyMs, web_version: SHARD_VERSION },
      { status: response.status }
    )
  } catch (error) {
    return NextResponse.json(
      {
        status: "degraded",
        readiness_reason: "backend_unreachable",
        ready_for_inference: false,
        active_browser_sessions: 0,
        active_scouts: 0,
        error: "Failed to connect to backend",
        backend_candidates: candidates,
        details: String(error),
        web_version: SHARD_VERSION,
      },
      { status: 200 }
    )
  }
}
