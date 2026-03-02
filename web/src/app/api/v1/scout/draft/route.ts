
import { NextRequest, NextResponse } from "next/server"
import {
  fetchWithBackendFailover,
  forwardRequestHeaders,
  shardBackendUrls,
} from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

const BASE_BACKEND_COOLDOWN_MS = 800
const MAX_BACKEND_COOLDOWN_MS = 8000
let backendCooldownUntilMs = 0
let consecutiveBackendFailures = 0

function noteBackendSuccess(): void {
  consecutiveBackendFailures = 0
  backendCooldownUntilMs = 0
}

function noteBackendFailure(): void {
  consecutiveBackendFailures = Math.min(10, consecutiveBackendFailures + 1)
  const cooldown = Math.min(
    MAX_BACKEND_COOLDOWN_MS,
    BASE_BACKEND_COOLDOWN_MS * Math.pow(2, Math.max(0, consecutiveBackendFailures - 1)),
  )
  backendCooldownUntilMs = Date.now() + cooldown
}

function inBackendCooldown(): boolean {
  return Date.now() < backendCooldownUntilMs
}

export async function POST(request: NextRequest) {
  const path = "/v1/scout/draft"
  const candidates = shardBackendUrls(path)

  if (inBackendCooldown()) {
    return NextResponse.json(
      {
        ok: false,
        transient_error: true,
        detail: "Backend in cooldown after recent failures; draft not accepted",
        backend_candidates: candidates,
      },
      { status: 200 },
    )
  }

  try {
    const body = await request.text()
    const { response, backend, attempts } = await fetchWithBackendFailover(path, {
      method: "POST",
      headers: forwardRequestHeaders(),
      body,
      timeoutMs: 4_000,
      totalTimeoutMs: 6_500,
      maxAttempts: 2,
      retryJitterMs: 180,
      failoverOnStatuses: [500, 502, 503, 504],
    })
    const data = await response.json().catch(() => ({}))
    if (!response.ok) {
      noteBackendFailure()
      return NextResponse.json(
        {
          ok: false,
          transient_error: true,
          detail: "Backend returned non-OK during draft submit",
          backend_status: response.status,
          backend,
          backend_attempts: attempts,
          ...data,
        },
        { status: 200 },
      )
    }
    noteBackendSuccess()
    return NextResponse.json({ ...data, backend, backend_attempts: attempts }, { status: 200 })
  } catch (error) {
    noteBackendFailure()
    // Preserve scout loop stability across short backend stalls.
    // Returning 200 + ok:false keeps client retry logic in control.
    return NextResponse.json(
      {
        ok: false,
        transient_error: true,
        detail: "Backend temporarily unavailable; draft not accepted",
        backend_candidates: candidates,
        error: String(error),
      },
      { status: 200 },
    )
  }
}

