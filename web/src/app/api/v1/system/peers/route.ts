import { NextResponse } from "next/server"
import { fetchWithBackendFailover, shardBackendUrls } from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

export async function GET() {
  const candidates = shardBackendUrls("/v1/system/peers")

  try {
    const { response, backend, attempts } = await fetchWithBackendFailover("/v1/system/peers", {
      timeoutMs: 30_000,
      failoverOnStatuses: [500, 502, 503, 504],
    })
    const data = await response.json()
    return NextResponse.json({ ...data, backend, backend_attempts: attempts }, { status: response.status })
  } catch (error) {
    return NextResponse.json(
      { peers: [], count: 0, status: "degraded", backend_candidates: candidates, error: String(error) },
      { status: 200 },
    )
  }
}
