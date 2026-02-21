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
    headers.delete("content-length") // Let fetch calculate this

    let body: BodyInit | null = null
    if (req.method !== "GET" && req.method !== "HEAD") {
        body = await req.arrayBuffer()
    }

    const res = await fetch(url, {
      method: req.method,
      headers: headers,
      body,
      cache: "no-store",
      duplex: 'half' // Required for Node.js fetch with bodies
    } as RequestInit)

    const resHeaders = new Headers(res.headers)
    resHeaders.delete("content-encoding")
    resHeaders.set("Access-Control-Allow-Origin", "*")
    resHeaders.set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
    resHeaders.set("Access-Control-Allow-Headers", "Content-Type, Authorization, X-Shard-Inference-Mode")

    return new NextResponse(res.body, {
      status: res.status,
      statusText: res.statusText,
      headers: resHeaders,
    })
  } catch (e: any) {
    console.error("Proxy error:", e)
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
