export const runtime = 'edge';
import { NextRequest, NextResponse } from "next/server"
import { fetchWithBackendFailover, shardBackendUrls } from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

export async function GET(request: NextRequest) {
  const search = request.nextUrl.search || ""
  const path = `/v1/pow/challenge${search}`
  const candidates = shardBackendUrls(path)

  try {
    const { response, backend, attempts } = await fetchWithBackendFailover(path, {
      method: "GET",
      timeoutMs: 10_000,
      failoverOnStatuses: [500, 502, 503, 504],
    })
    const data = await response.json().catch(() => ({}))
    return NextResponse.json(
      { ...data, backend, backend_attempts: attempts },
      { status: response.status },
    )
  } catch (error) {
    return NextResponse.json(
      {
        ok: false,
        detail: "Failed to request PoW challenge from backend",
        backend_candidates: candidates,
        error: String(error),
      },
      { status: 502 },
    )
  }
}


