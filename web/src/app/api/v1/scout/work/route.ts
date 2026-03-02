export const runtime = 'edge';
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

export async function GET(request: NextRequest) {
  const search = request.nextUrl.search || ""
  const path = `/v1/scout/work${search}`
  const candidates = shardBackendUrls(path)

  if (inBackendCooldown()) {
    return NextResponse.json(
      {
        work: null,
        ok: true,
        transient_error: true,
        detail: "Backend in cooldown after recent failures; returning empty work",
        backend_candidates: candidates,
      },
      { status: 200 },
    )
  }

  try {
    const { response, backend, attempts } = await fetchWithBackendFailover(path, {
      method: "GET",
      headers: forwardRequestHeaders(),
      timeoutMs: 2_500,
      totalTimeoutMs: 3_800,
      maxAttempts: 2,
      retryJitterMs: 180,
      failoverOnStatuses: [500, 502, 503, 504, 521, 530],
    })
    const data = await response.json().catch(() => ({}))
    if (!response.ok) {
      noteBackendFailure()
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
    noteBackendSuccess()
    return NextResponse.json({ ...data, backend, backend_attempts: attempts }, { status: 200 })
  } catch (error) {
    noteBackendFailure()
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

