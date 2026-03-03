export const runtime = 'edge';
import { NextRequest, NextResponse } from "next/server"
import {
  forwardRequestHeaders,
  shardBackendUrls,
} from "@/lib/server/shard-backend"
import { recordChatProxyResult } from "@/lib/server/proxy-chat-sli"

export const dynamic = "force-dynamic"

function parseTimeoutMs(raw: string | undefined, fallback: number): number {
  const parsed = Number.parseInt(raw ?? "", 10)
  if (!Number.isFinite(parsed) || parsed <= 0) return fallback
  return parsed
}

const CHAT_TIMEOUT_MS = parseTimeoutMs(
  process.env.SHARD_CHAT_PRIMARY_TIMEOUT_MS,
  65000,
)

export async function GET() {
  return NextResponse.json({
    message: "Use POST to send chat messages",
    format: "{ model: string, messages: { role: string, content: string }[] }",
  })
}

export async function POST(request: NextRequest) {
  const candidates = shardBackendUrls("/v1/chat/completions")
  const startedAt = Date.now()

  try {
    const bodyText = await request.text()
    const headers = forwardRequestHeaders()
    const attempts: string[] = []
    let response: Response | null = null
    let backendUsed: string | null = null
    let lastFailureKind: "http_5xx" | "timeout" | "other_error" = "other_error"
    let lastBackendStatus: number | null = null

    for (let i = 0; i < candidates.length; i += 1) {
      const candidate = candidates[i]
      attempts.push(candidate)

      try {
        const candidateResponse = await fetch(candidate, {
          method: "POST",
          headers,
          body: bodyText,
          signal: AbortSignal.timeout(CHAT_TIMEOUT_MS),
          cache: "no-store",
        })

        if (candidateResponse.ok || candidateResponse.status < 500) {
          response = candidateResponse
          backendUsed = candidate
          lastBackendStatus = candidateResponse.status
          break
        }
        lastFailureKind = "http_5xx"
        lastBackendStatus = candidateResponse.status
      } catch (error) {
        const errorName = error instanceof Error ? error.name : ""
        if (errorName === "TimeoutError") {
          lastFailureKind = "timeout"
        } else {
          lastFailureKind = "other_error"
        }
        console.error("[Chat Proxy] Candidate fetch exception:", candidate, error)
        continue
      }
    }

    if (!response || !backendUsed) {
      recordChatProxyResult({
        outcome: lastFailureKind,
        attempts: attempts.length,
        fallback_used: false,
        latency_ms: Date.now() - startedAt,
      })
      return NextResponse.json(
        {
          error: "Chat completion failed after failover attempts",
          backend_attempts: attempts,
          backend_status: lastBackendStatus ?? undefined,
        },
        { status: 502 },
      )
    }

    recordChatProxyResult({
      outcome: response.ok ? "success" : response.status >= 500 ? "http_5xx" : "other_error",
      attempts: attempts.length,
      fallback_used: false,
      latency_ms: Date.now() - startedAt,
    })

    if (response.body) {
      return new NextResponse(response.body, {
        status: response.status,
        headers: {
          "Content-Type": response.headers.get("content-type") || "text/event-stream",
          "Cache-Control": "no-cache",
          Connection: "keep-alive",
          "X-Shard-Backend": backendUsed,
          "X-Shard-Backend-Attempts": String(attempts.length),
          "Access-Control-Allow-Origin": "*",
        },
      })
    }

    const data = await response.json()
    return NextResponse.json(
      { ...data, backend: backendUsed, backend_attempts: attempts },
      { status: response.status },
    )
  } catch (error) {
    recordChatProxyResult({
      outcome: "other_error",
      attempts: 1,
      fallback_used: false,
      latency_ms: Date.now() - startedAt,
    })
    return NextResponse.json(
      {
        error: "Chat completion failed after failover attempts",
        details: String(error),
      },
      { status: 502 }
    )
  }
}

export async function OPTIONS() {
  return new NextResponse(null, {
    status: 204,
    headers: {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
      "Access-Control-Allow-Headers": "Content-Type, Authorization, X-Shard-Inference-Mode, X-Shard-Wallet",
    },
  })
}

