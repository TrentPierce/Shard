import { NextRequest, NextResponse } from "next/server"
import {
  fetchWithBackendFailover,
  forwardRequestHeaders,
  preferredBackendCandidatesFromHeaders,
} from "@/lib/server/shard-backend"
import {
  buildPreflightResponse,
  corsHeadersForRequest,
  resolveCorsOrigin,
} from "@/lib/server/cors"

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
  if (request.headers.get("origin") && !resolveCorsOrigin(request)) {
    return NextResponse.json({ ok: false, detail: "Origin not allowed" }, { status: 403 })
  }

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
      headers: {
        ...responseHeaders(backend, attempts),
        ...corsHeadersForRequest(request),
      },
    })
  } catch (error) {
    return NextResponse.json(
      {
        ok: false,
        detail: String((error as Error)?.message ?? error ?? "upstream request failed"),
      },
      { status: 502, headers: corsHeadersForRequest(request) },
    )
  }
}

export async function proxyShardJsonPost(
  request: NextRequest,
  backendPath: string,
): Promise<NextResponse> {
  if (request.headers.get("origin") && !resolveCorsOrigin(request)) {
    return NextResponse.json({ ok: false, detail: "Origin not allowed" }, { status: 403 })
  }

  try {
    const bodyText = await request.text()
    const { response, backend, attempts } = await fetchWithBackendFailover(backendPath, {
      method: "POST",
      headers: forwardRequestHeaders(),
      body: bodyText,
      timeoutMs: JSON_PROXY_TIMEOUT_MS,
      totalTimeoutMs: JSON_PROXY_TOTAL_TIMEOUT_MS,
      preferredCandidates: preferredBackendCandidatesFromHeaders(backendPath),
    })
    const payload = await response.json().catch(() => ({}))
    return NextResponse.json(payload, {
      status: response.status,
      headers: {
        ...responseHeaders(backend, attempts),
        ...corsHeadersForRequest(request),
      },
    })
  } catch (error) {
    return NextResponse.json(
      {
        ok: false,
        detail: String((error as Error)?.message ?? error ?? "upstream request failed"),
      },
      { status: 502, headers: corsHeadersForRequest(request) },
    )
  }
}

export function proxyOptions(request: NextRequest, methods: string) {
  return buildPreflightResponse(request, methods)
}
