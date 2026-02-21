import { NextRequest, NextResponse } from "next/server"

// Use HTTP to avoid SSL certificate issues with nip.io self-signed cert
const BACKEND_HOST = "35.175.242.222"
const BACKEND_PORT = 9091

async function proxy(req: NextRequest, { params }: { params: { path: string[] } }) {
  const path = params.path?.join("/") ?? ""
  const search = req.nextUrl.search
  const targetUrl = `http://${BACKEND_HOST}:${BACKEND_PORT}/${path}${search}`

  try {
    const headers: Record<string, string> = {}
    req.headers.forEach((value, key) => {
      if (!["host", "connection", "content-length"].includes(key.toLowerCase())) {
        headers[key] = value
      }
    })

    const fetchOptions: RequestInit = {
      method: req.method,
      headers,
      cache: "no-store",
    }

    // Add timeout
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), 8000)
    fetchOptions.signal = controller.signal

    if (req.method !== "GET" && req.method !== "HEAD") {
      const bodyText = await req.text()
      fetchOptions.body = bodyText
    }

    const response = await fetch(targetUrl, fetchOptions)
    clearTimeout(timeout)

    const responseHeaders = new Headers(response.headers)
    responseHeaders.delete("content-encoding")
    responseHeaders.set("Access-Control-Allow-Origin", "*")

    const contentType = response.headers.get("content-type") || ""
    if (contentType.includes("application/json")) {
      const jsonData = await response.json()
      return NextResponse.json(jsonData, {
        status: response.status,
        headers: responseHeaders,
      })
    } else {
      const textData = await response.text()
      return new NextResponse(textData, {
        status: response.status,
        headers: responseHeaders,
      })
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : "Unknown proxy error"
    console.error("Shard API proxy error:", message, "targetUrl:", targetUrl)
    return NextResponse.json(
      {
        error: "Proxy request failed",
        target: targetUrl,
        details: message,
      },
      { status: 502 }
    )
  }
}

export const GET = proxy
export const POST = proxy
export const PUT = proxy
export const PATCH = proxy
export const DELETE = proxy
export const HEAD = proxy

export async function OPTIONS() {
  return new NextResponse(null, {
    status: 204,
    headers: {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Methods": "GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS",
      "Access-Control-Allow-Headers": "Content-Type, Authorization, X-Shard-Inference-Mode",
    },
  })
}
