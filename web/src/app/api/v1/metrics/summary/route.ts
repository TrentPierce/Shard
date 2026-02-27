import { NextResponse } from "next/server"
import { fetchWithBackendFailover, shardBackendUrls } from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

export async function GET() {
  // The Rust daemon exposes metrics at `/metrics/summary` (no `/v1` prefix).
  // We proxy that here for the web app and dashboards.
  const candidates = shardBackendUrls("/metrics/summary")

  try {
    const { response, backend, attempts } = await fetchWithBackendFailover("/metrics/summary", {
      timeoutMs: 6_000,
      failoverOnStatuses: [500, 502, 503, 504],
    })
    const data = await response.json().catch(() => ({}))
    if (!response.ok) {
      return NextResponse.json(
        {
          status: "degraded",
          backend_status: response.status,
          backend,
          backend_attempts: attempts,
          ...data,
        },
        { status: 200 },
      )
    }
    return NextResponse.json({ ...data, backend, backend_attempts: attempts }, { status: 200 })
  } catch (error) {
    return NextResponse.json(
      {
        status: "degraded",
        backend_candidates: candidates,
        error: String(error),
      },
      { status: 200 },
    )
  }
}
