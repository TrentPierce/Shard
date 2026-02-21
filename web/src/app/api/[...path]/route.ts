import { NextRequest, NextResponse } from "next/server"

const BACKEND_URL = "https://35.175.242.222.nip.io"

async function proxy(req: NextRequest, { params }: { params: { path: string[] } }) {
  const path = params.path.join("/")
  const search = req.nextUrl.search
  const url = `${BACKEND_URL}/${path}${search}`

  try {
    const headers = new Headers(req.headers)
    headers.delete("host")
    headers.delete("connection")

    const body = req.method !== "GET" && req.method !== "HEAD" ? await req.blob() : null

    const res = await fetch(url, {
      method: req.method,
      headers: headers,
      body,
      cache: "no-store",
    })

    const resHeaders = new Headers(res.headers)
    resHeaders.delete("content-encoding") // Let Vercel handle compression
    resHeaders.set("Access-Control-Allow-Origin", "*")
    resHeaders.set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
    resHeaders.set("Access-Control-Allow-Headers", "Content-Type, Authorization, X-Shard-Inference-Mode")

    return new NextResponse(res.body, {
      status: res.status,
      statusText: res.statusText,
      headers: resHeaders,
    })
  } catch (e: any) {
    return NextResponse.json({ error: "Proxy failed", details: e.message }, { status: 502 })
  }
}

export const GET = proxy
export const POST = proxy
export const OPTIONS = async () => {
  return new NextResponse(null, {
    status: 204,
    headers: {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
      "Access-Control-Allow-Headers": "Content-Type, Authorization, X-Shard-Inference-Mode",
    },
  })
}
