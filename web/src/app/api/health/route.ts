import { NextResponse } from "next/server"
import { shardBackendUrl } from "@/lib/server/shard-backend"
import { SHARD_VERSION } from "@/lib/version"

export const dynamic = "force-dynamic"

export async function GET() {
  const url = shardBackendUrl("/health")

  try {
    const started = performance.now()
    const response = await fetch(url, {
      signal: AbortSignal.timeout(8000),
      cache: "no-store",
    })
    const latencyMs = Math.round(performance.now() - started)
    const data = await response.json()
    return NextResponse.json(
      { ...data, backend: url, latency_ms: latencyMs, web_version: SHARD_VERSION },
      { status: response.status }
    )
  } catch (error) {
    return NextResponse.json(
      {
        status: "unreachable",
        error: "Failed to connect to backend",
        backend: url,
        details: String(error),
      },
      { status: 502 }
    )
  }
}
