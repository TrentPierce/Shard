import { NextRequest, NextResponse } from "next/server"
import {
  fetchWithBackendFailover,
  forwardRequestHeaders,
  shardBackendUrls,
} from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

export async function GET(request: NextRequest) {
  const search = request.nextUrl.search || ""
  const path = `/v1/scout/work${search}`
  const candidates = shardBackendUrls(path)

  try {
    const { response, backend, attempts } = await fetchWithBackendFailover(path, {
      method: "GET",
      headers: forwardRequestHeaders(),
      timeoutMs: 8_000,
      failoverOnStatuses: [500, 502, 503, 504],
    })
    const data = await response.json().catch(() => ({}))
    if (!response.ok) {
      return NextResponse.json(
        {
          work: null,
          ok: true,
          transient_error: true,
          detail: "Backend returned non-OK while polling scout work",
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
    // Keep scout polling loops alive during transient backend failures.
    // Returning empty work (200) prevents clients from entering hard-failure paths.
    return NextResponse.json(
      {
        work: null,
        ok: true,
        transient_error: true,
        detail: "Backend temporarily unavailable; returning empty work",
        backend_candidates: candidates,
        error: String(error),
      },
      { status: 200 },
    )
  }
}
