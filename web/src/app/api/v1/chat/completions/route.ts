import { NextRequest, NextResponse } from "next/server"
import {
  forwardRequestHeaders,
  shardBackendUrls,
} from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

const TTFT_TIMEOUT_MS = 2000 // 2 second threshold for "Enterprise" stability

export async function GET() {
  return NextResponse.json({
    message: "Use POST to send chat messages",
    format: "{ model: string, messages: { role: string, content: string }[] }",
  })
}

export async function POST(request: NextRequest) {
  const primaryCandidates = shardBackendUrls("/v1/chat/completions", false)
  const fallbackCandidates = shardBackendUrls("/v1/chat/completions", true)
  const allCandidates = Array.from(new Set([...primaryCandidates, ...fallbackCandidates]))

  try {
    const bodyText = await request.text()
    const headers = forwardRequestHeaders()
    const attempts: string[] = []
    let usedFallback = false
    let response: Response | null = null
    let backendUsed: string | null = null

    for (let i = 0; i < allCandidates.length; i += 1) {
      const candidate = allCandidates[i]
      attempts.push(candidate)
      const isFallbackCandidate = !primaryCandidates.includes(candidate)

      const controller = new AbortController()
      const candidateRequest = fetch(candidate, {
        method: "POST",
        headers,
        body: bodyText,
        signal: controller.signal,
        cache: "no-store",
      })

      const timeoutMs = isFallbackCandidate ? 60000 : TTFT_TIMEOUT_MS
      const timeoutPromise = new Promise<null>((resolve) =>
        setTimeout(() => resolve(null), timeoutMs),
      )

      let candidateResponse: Response | null = null
      try {
        candidateResponse = await Promise.race([candidateRequest, timeoutPromise])
      } catch (error) {
        console.error("[Enterprise Guard] Candidate fetch exception:", candidate, error)
        candidateResponse = null
      }

      if (candidateResponse === null) {
        controller.abort()
        continue
      }

      if (candidateResponse.ok) {
        response = candidateResponse
        backendUsed = candidate
        usedFallback = isFallbackCandidate
        break
      }

      if (candidateResponse.status < 500) {
        response = candidateResponse
        backendUsed = candidate
        usedFallback = isFallbackCandidate
        break
      }
    }

    if (!response || !backendUsed) {
      return NextResponse.json(
        {
          error: "Chat completion failed after failover attempts",
          backend_attempts: attempts,
        },
        { status: 502 },
      )
    }

    if (response.body) {
      return new NextResponse(response.body, {
        status: response.status,
        headers: {
          "Content-Type": response.headers.get("content-type") || "text/event-stream",
          "Cache-Control": "no-cache",
          Connection: "keep-alive",
          "X-Shard-Fallback": usedFallback ? "true" : "false",
          "X-Shard-Backend": backendUsed,
          "Access-Control-Allow-Origin": "*",
        },
      })
    }

    const data = await response.json()
    return NextResponse.json(
      { ...data, backend: backendUsed, backend_attempts: attempts, fallback_used: usedFallback },
      { status: response.status },
    )
  } catch (error) {
    return NextResponse.json(
      {
        error: "Chat completion failed after fallback attempt",
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
