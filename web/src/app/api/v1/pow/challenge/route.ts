export const runtime = 'edge';
import { NextRequest, NextResponse } from "next/server"
import {
  fetchWithBackendFailover,
  forwardRequestHeaders,
  preferredBackendCandidatesFromHeaders,
  shardBackendUrls,
} from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

export async function GET(request: NextRequest) {
  const search = request.nextUrl.search || ""
  const path = `/v1/pow/challenge${search}`
  const candidates = shardBackendUrls(path)
  const preferredCandidates = preferredBackendCandidatesFromHeaders(path)

  try {
    const { response, backend, attempts } = await fetchWithBackendFailover(path, {
      method: "GET",
      headers: forwardRequestHeaders(),
      maxAttempts: preferredCandidates?.length || 3,
      preferredCandidates,
      loadAware: !preferredCandidates,
      timeoutMs: 10_000,
      failoverOnStatuses: [500, 502, 503, 504, 521, 530],
    })
    const payloadText = await response.text().catch(() => "")
    let data: Record<string, unknown> = {}
    if (payloadText.trim()) {
      try {
        data = JSON.parse(payloadText)
      } catch {
        data = { raw_body_preview: payloadText.slice(0, 400) }
      }
    }
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


