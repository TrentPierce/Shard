import { NextRequest, NextResponse } from "next/server"
import { forwardRequestHeaders, shardBackendUrl } from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

const TTFT_TIMEOUT_MS = 2000 // 2 second threshold for "Enterprise" stability

export async function GET() {
  return NextResponse.json({
    message: "Use POST to send chat messages",
    format: "{ model: string, messages: { role: string, content: string }[] }",
  })
}

export async function POST(request: NextRequest) {
  const primaryUrl = shardBackendUrl("/v1/chat/completions", false)
  const fallbackUrl = shardBackendUrl("/v1/chat/completions", true)

  try {
    const bodyText = await request.text()
    const headers = forwardRequestHeaders()

    // 1. Start the primary request
    const primaryController = new AbortController()
    const primaryPromise = fetch(primaryUrl, {
      method: "POST",
      headers,
      body: bodyText,
      signal: primaryController.signal,
      cache: "no-store",
    })

    // 2. Race the primary against a timeout
    const timeoutPromise = new Promise<null>((resolve) => 
      setTimeout(() => resolve(null), TTFT_TIMEOUT_MS)
    )

    const result = await Promise.race([primaryPromise, timeoutPromise])

    let response: Response
    let usedFallback = false

    if (result === null || !result.ok) {
      // Primary timed out or failed immediately - trigger fallback
      console.log(`[Enterprise Guard] Primary stalled or failed. Routing to fallback: ${fallbackUrl}`)
      usedFallback = true
      primaryController.abort() // Cancel primary if it's still hanging
      
      response = await fetch(fallbackUrl, {
        method: "POST",
        headers,
        body: bodyText,
        signal: AbortSignal.timeout(60000),
        cache: "no-store",
      })
    } else {
      response = result
    }

    if (response.body) {
      return new NextResponse(response.body, {
        status: response.status,
        headers: {
          "Content-Type": response.headers.get("content-type") || "text/event-stream",
          "Cache-Control": "no-cache",
          Connection: "keep-alive",
          "X-Shard-Fallback": usedFallback ? "true" : "false",
          "Access-Control-Allow-Origin": "*",
        },
      })
    }

    const data = await response.json()
    return NextResponse.json(data, { status: response.status })
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
