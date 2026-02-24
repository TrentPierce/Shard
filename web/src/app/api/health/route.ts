import { NextResponse } from "next/server"
import { fetchWithBackendFailover, shardBackendUrls } from "@/lib/server/shard-backend"
import { SHARD_VERSION } from "@/lib/version"

export const dynamic = "force-dynamic"

export async function GET() {
  const candidates = shardBackendUrls("/health")

  try {
    const started = performance.now()
    const { response, backend, attempts } = await fetchWithBackendFailover("/health")
    const latencyMs = Math.round(performance.now() - started)
    const data = await response.json()
    return NextResponse.json(
      { ...data, backend, backend_attempts: attempts, latency_ms: latencyMs, web_version: SHARD_VERSION },
      { status: response.status }
    )
  } catch (error) {
    return NextResponse.json(
      {
        status: "unreachable",
        error: "Failed to connect to backend",
        backend_candidates: candidates,
        details: String(error),
      },
      { status: 502 }
    )
  }
}
