
import { NextRequest, NextResponse } from "next/server"
import {
  fetchWithBackendFailover,
  forwardRequestHeaders,
  shardBackendUrls,
} from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

export async function POST(request: NextRequest) {
  const path = "/browser-layer/submit"
  const candidates = shardBackendUrls(path)

  try {
    const body = await request.text()
    const { response, backend, attempts } = await fetchWithBackendFailover(path, {
      method: "POST",
      headers: forwardRequestHeaders(),
      body,
      timeoutMs: 8_000,
      failoverOnStatuses: [500, 502, 503, 504],
    })
    const data = await response.json().catch(() => ({}))
    if (!response.ok) {
      return NextResponse.json(
        {
          ok: false,
          transient_error: true,
          detail: "Backend returned non-OK for browser-layer submit",
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
        ok: false,
        transient_error: true,
        detail: "Failed to submit browser layer output",
        backend_candidates: candidates,
        error: String(error),
      },
      { status: 200 },
    )
  }
}

