import { NextRequest, NextResponse } from "next/server"

const BACKEND_ORIGIN = "https://35.175.242.222.nip.io:9091"

async function proxy(req: NextRequest, { params }: { params: { path: string[] } }) {
  const path = params.path?.join("/") ?? ""
  const search = req.nextUrl.search
  const targetUrl = `${BACKEND_ORIGIN}/${path}${search}`

  try {
    const headers = new Headers(req.headers)
    headers.delete("host")
    headers.delete("connection")
    headers.delete("content-length")

    const body = req.method === "GET" || req.method === "HEAD" ? undefined : req.body

    const response = await fetch(targetUrl, {
      method: req.method,
      headers,
      body,
      cache: "no-store",
      redirect: "manual",
      duplex: body ? "half" : undefined,
    } as RequestInit)

    const responseHeaders = new Headers(response.headers)
    responseHeaders.delete("content-encoding")

    return new NextResponse(response.body, {
      status: response.status,
      statusText: response.statusText,
      headers: responseHeaders,
    })
  } catch (error) {
    const message = error instanceof Error ? error.message : "Unknown proxy error"
    console.error("Shard API proxy error", { targetUrl, message })
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
      Allow: "GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS",
    },
  })
}
