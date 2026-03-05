export const runtime = 'edge';
import { NextResponse } from "next/server"
import { shardBackendUrls } from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

export async function GET() {
  const url = shardBackendUrls("/health")[0] ?? "http://127.0.0.1:9091/health"

  try {
    const response = await fetch(url, {
      signal: AbortSignal.timeout(5000),
      cache: "no-store",
    })
    const data = await response.json()
    return NextResponse.json({
      status: "ok",
      backend: url,
      backend_status: data.status,
      version: data.version,
      peer_id: data.peer_id,
    })
  } catch (error) {
    return NextResponse.json({
      status: "error",
      backend: url,
      error: String(error),
    })
  }
}

export async function POST() {
  return GET()
}

