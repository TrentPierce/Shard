import { NextRequest, NextResponse } from "next/server"

const EC2_URL = "http://35.175.242.222:9091"

export async function GET(request: NextRequest) {
  return proxy(request)
}

export async function POST(request: NextRequest) {
  return proxy(request)
}

async function proxy(req: NextRequest): Promise<NextResponse> {
  const path = req.nextUrl.pathname.replace(/^\/api\//, "")
  const url = `${EC2_URL}/${path}${req.nextUrl.search}`

  try {
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), 8000)

    const body = req.method !== "GET" && req.method !== "HEAD" 
      ? await req.text() 
      : undefined

    const response = await fetch(url, {
      method: req.method,
      headers: {
        "Content-Type": "application/json",
      },
      body,
      signal: controller.signal,
    })

    clearTimeout(timeout)

    const data = await response.json()

    return NextResponse.json(data, {
      status: response.status,
      headers: {
        "Access-Control-Allow-Origin": "*",
      },
    })
  } catch (error) {
    return NextResponse.json({
      error: "Proxy failed",
      details: String(error),
      url,
    }, { status: 502 })
  }
}

export async function OPTIONS() {
  return new NextResponse(null, {
    status: 204,
    headers: {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
      "Access-Control-Allow-Headers": "Content-Type, Authorization, X-Shard-Inference-Mode",
    },
  })
}
