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
  const path = "/v1/scout/client-event"
  const candidates = shardBackendUrls(path)
  const preferredCandidates = preferredBackendCandidatesFromHeaders(path)

  try {
    const body = await request.text()
    const { response, backend, attempts } = await fetchWithBackendFailover(path, {
      method: "POST",
      headers: forwardRequestHeaders(),
      body,
      timeoutMs: 1_200,
      totalTimeoutMs: 2_800,
      // Allow one failover hop so misconfigured primaries do not drop scout telemetry.
      maxAttempts: preferredCandidates?.length || 2,
      preferredCandidates,
      loadAware: !preferredCandidates,
      failoverOnStatuses: [500, 502, 503, 504, 521, 530],
    })
    const data = await response.json().catch(() => ({}))
    if (response.status >= 500) {
      return NextResponse.json(
        {
          ok: false,
          transient_error: true,
          detail: "Telemetry backend unavailable",
          backend_status: response.status,
          backend,
          backend_attempts: attempts,
          ...data,
        },
        { status: 202 },
      )
    }
    return NextResponse.json(
      { ...data, backend, backend_attempts: attempts },
      { status: response.status },
    )
  } catch (error) {
    return NextResponse.json(
      {
        ok: false,
        transient_error: true,
        detail: "Failed to submit scout client event to backend",
        backend_candidates: candidates,
        error: String(error),
      },
      { status: 202 },
    )
  }
}

