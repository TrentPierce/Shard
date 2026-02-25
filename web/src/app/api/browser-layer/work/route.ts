import { NextRequest, NextResponse } from "next/server"
import {
  fetchWithBackendFailover,
  forwardRequestHeaders,
  shardBackendUrls,
} from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

export async function GET(request: NextRequest) {
  const search = request.nextUrl.search || ""
  const path = `/browser-layer/work${search}`
  const candidates = shardBackendUrls(path)

  try {
    const { response, backend, attempts } = await fetchWithBackendFailover(path, {
      method: "GET",
      headers: forwardRequestHeaders(),
      timeoutMs: 20_000,
      failoverOnStatuses: [500, 502, 503, 504],
    })
    const data = await response.json().catch(() => ({}))
    return NextResponse.json(
      { ...data, backend, backend_attempts: attempts },
      { status: response.status },
    )
  } catch (error) {
    // Keep browser scouts alive during transient backend issues.
    // Returning an empty work response avoids hard-failing client loops.
    return NextResponse.json(
      {
        ok: true,
        status: "empty",
        transient_error: true,
        detail: "Backend temporarily unavailable; returning empty work",
        backend_candidates: candidates,
        error: String(error),
      },
      { status: 200 },
    )
  }
}
