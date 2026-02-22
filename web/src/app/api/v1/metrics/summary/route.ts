import { NextResponse } from "next/server"
import { shardBackendUrl } from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

export async function GET() {
  const url = shardBackendUrl("/v1/metrics/summary")

  try {
    const response = await fetch(url, {
      signal: AbortSignal.timeout(8000),
      cache: "no-store",
    })
    const data = await response.json()
    return NextResponse.json(data, { status: response.status })
  } catch (error) {
    return NextResponse.json({ tokens_verified_24h: 0, status: "degraded", backend: url, error: String(error) }, { status: 502 })
  }
}
