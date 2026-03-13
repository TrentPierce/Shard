import { NextRequest, NextResponse } from "next/server"
import {
  fetchWithBackendFailover,
  forwardRequestHeaders,
  preferredBackendCandidatesFromHeaders,
} from "@/lib/server/shard-backend"

function parseTimeoutMs(raw: string | undefined, fallback: number): number {
  const parsed = Number.parseInt(raw ?? "", 10)
  if (!Number.isFinite(parsed) || parsed <= 0) return fallback
  return parsed
}

const JSON_PROXY_TIMEOUT_MS = parseTimeoutMs(
  process.env.SHARD_JSON_PROXY_TIMEOUT_MS,
  20_000,
)
const JSON_PROXY_TOTAL_TIMEOUT_MS = parseTimeoutMs(
  process.env.SHARD_JSON_PROXY_TOTAL_TIMEOUT_MS,
  45_000,
)

function responseHeaders(backend: string, attempts: number): HeadersInit {
  return {
    "X-Shard-Backend": backend,
    "X-Shard-Backend-Attempts": String(attempts),
  }
}

export async function proxyShardJsonGet(
  request: NextRequest,
  backendPath: string,
): Promise<NextResponse> {
  try {
    const { response, backend, attempts } = await fetchWithBackendFailover(backendPath, {
      method: "GET",
      headers: forwardRequestHeaders(),
      timeoutMs: JSON_PROXY_TIMEOUT_MS,
      totalTimeoutMs: JSON_PROXY_TOTAL_TIMEOUT_MS,
      preferredCandidates: preferredBackendCandidatesFromHeaders(backendPath),
    })
    const payload = await response.json().catch(() => ({}))
    return NextResponse.json(payload, {
      status: response.status,
      headers: responseHeaders(backend, attempts),
    })
  } catch (error) {
    return NextResponse.json(
      {
        ok: false,
        detail: String((error as Error)?.message ?? error ?? "upstream request failed"),
      },
      { status: 502 },
    )
  }
}

export async function proxyShardJsonPost(
  request: NextRequest,
  backendPath: string,
): Promise<NextResponse> {
  try {
    const bodyText = await request.text()
    const { response, backend, attempts } = await fetchWithBackendFailover(backendPath, {
      method: "POST",
      headers: forwardRequestHeaders(),
      body: bodyText,
      timeoutMs: JSON_PROXY_TIMEOUT_MS,
      totalTimeoutMs: JSON_PROXY_TOTAL_TIMEOUT_MS,
      preferredCandidates: preferredBackendCandidatesFromHeaders(backendPath),
      failoverOnStatuses: [500, 502, 503, 504, 521, 530],
    })
    const payload = await response.json().catch(() => ({}))
    return NextResponse.json(payload, {
      status: response.status,
      headers: responseHeaders(backend, attempts),
    })
  } catch (error) {
    return NextResponse.json(
      {
        ok: false,
        detail: String((error as Error)?.message ?? error ?? "upstream request failed"),
      },
      { status: 502 },
    )
  }
}
