import { NextRequest, NextResponse } from "next/server"

export const dynamic = "force-dynamic"

const EC2_URL = "http://35.175.242.222:9091"

export async function POST(request: NextRequest) {
  const url = `${EC2_URL}/v1/chat/completions`
  
  try {
    const body = await request.text()
    
    const response = await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body,
      signal: AbortSignal.timeout(60000),
    })

    // Handle streaming response
    if (response.body) {
      const reader = response.body.getReader()
      const encoder = new TextDecoder()

      const stream = new ReadableStream({
        async start(controller) {
          try {
            while (true) {
              const { done, value } = await reader.read()
              if (done) break
              controller.enqueue(value)
            }
          } catch (e) {
            controller.error(e)
          } finally {
            reader.releaseLock()
          }
        },
      })

      return new NextResponse(stream, {
        status: response.status,
        headers: {
          "Content-Type": "text/event-stream",
          "Cache-Control": "no-cache",
          "Access-Control-Allow-Origin": "*",
        },
      })
    }

    // Non-streaming response
    const data = await response.json()
    return NextResponse.json(data, {
      status: response.status,
    })
  } catch (error) {
    return NextResponse.json({
      error: "Chat completion failed",
      details: String(error),
    }, { status: 502 })
  }
}

export async function OPTIONS() {
  return new NextResponse(null, {
    status: 204,
    headers: {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Methods": "POST, OPTIONS",
      "Access-Control-Allow-Headers": "Content-Type, Authorization",
    },
  })
}
