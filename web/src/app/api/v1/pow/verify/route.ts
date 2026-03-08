export const runtime = 'edge';
import { NextRequest, NextResponse } from "next/server"
import {
  fetchWithBackendFailover,
  forwardRequestHeaders,
  preferredBackendCandidatesFromHeaders,
  shardBackendUrls,
} from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

export async function POST(request: NextRequest) {
  const path = "/v1/pow/verify"
  const candidates = shardBackendUrls(path)
  const preferredCandidates = preferredBackendCandidatesFromHeaders(path)

  try {
    const body = await request.text()
    const { response, backend, attempts } = await fetchWithBackendFailover(path, {
      method: "POST",
      headers: forwardRequestHeaders(),
      body,
      maxAttempts: preferredCandidates?.length || 3,
      preferredCandidates,
      loadAware: !preferredCandidates,
      timeoutMs: 15_000,
      failoverOnStatuses: [500, 502, 503, 504, 521, 530],
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
        detail: "Failed to verify PoW solution with backend",
        backend_candidates: candidates,
        error: String(error),
      },
      { status: 502 },
    )
  }
}


