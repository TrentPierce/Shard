import { NextRequest, NextResponse } from "next/server"
import { forwardRequestHeaders, shardBackendUrl } from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

export async function GET() {
  return NextResponse.json({
    message: "Use POST to send chat messages",
    format: "{ model: string, messages: { role: string, content: string }[] }",
  })
}

export async function POST(request: NextRequest) {
  const url = shardBackendUrl("/v1/chat/completions")

  try {
    const body = await request.text()
    const response = await fetch(url, {
      method: "POST",
      headers: forwardRequestHeaders(),
      body,
      signal: AbortSignal.timeout(120000),
      cache: "no-store",
    })

    if (response.body) {
      return new NextResponse(response.body, {
        status: response.status,
        headers: {
          "Content-Type": response.headers.get("content-type") || "text/event-stream",
          "Cache-Control": "no-cache",
          Connection: "keep-alive",
          "Access-Control-Allow-Origin": "*",
        },
      })
    }

    const data = await response.json()
    return NextResponse.json(data, { status: response.status })
  } catch (error) {
    return NextResponse.json(
      {
        error: "Chat completion failed",
        backend: url,
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
