export const runtime = "edge"

import { NextRequest, NextResponse } from "next/server"
import {
  fetchWithBackendFailover,
  forwardRequestHeaders,
  shardBackendUrls,
} from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

export async function POST(request: NextRequest) {
  const path = "/v1/telemetry/webgpu"
  const candidates = shardBackendUrls(path)

  try {
    const body = await request.text()
    const { response, backend, attempts } = await fetchWithBackendFailover(path, {
      method: "POST",
      headers: forwardRequestHeaders(),
      body,
      timeoutMs: 1_500,
      totalTimeoutMs: 2_000,
      maxAttempts: 1,
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
        transient_error: true,
        detail: "Failed to submit WebGPU telemetry to backend",
        backend_candidates: candidates,
        error: String(error),
      },
      { status: 202 },
    )
  }
}

export async function GET() {
  const path = "/metrics/webgpu-coverage"
  try {
    const { response, backend, attempts } = await fetchWithBackendFailover(path, {
      method: "GET",
      headers: forwardRequestHeaders(),
      timeoutMs: 2_000,
      totalTimeoutMs: 2_500,
      maxAttempts: 2,
      failoverOnStatuses: [500, 502, 503, 504, 521, 530],
    })
    const data = await response.json().catch(() => ({}))
    return NextResponse.json(
      { ...data, backend, backend_attempts: attempts },
      { status: response.status },
    )
  } catch (error) {
    return NextResponse.json(
      { ok: false, detail: "Failed to fetch WebGPU coverage", error: String(error) },
      { status: 502 },
    )
  }
}
