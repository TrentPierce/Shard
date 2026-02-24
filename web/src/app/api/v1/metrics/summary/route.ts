import { NextResponse } from "next/server"
import { fetchWithBackendFailover, shardBackendUrls } from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

export async function GET() {
  // The Rust daemon exposes metrics at `/metrics/summary` (no `/v1` prefix).
  // We proxy that here for the web app and dashboards.
  const candidates = shardBackendUrls("/metrics/summary")

  try {
    const { response, backend, attempts } = await fetchWithBackendFailover("/metrics/summary")
    const data = await response.json()
    return NextResponse.json({ ...data, backend, backend_attempts: attempts }, { status: response.status })
  } catch (error) {
    return NextResponse.json(
      { tokens_verified_24h: 0, status: "degraded", backend_candidates: candidates, error: String(error) },
      { status: 502 },
    )
  }
}
