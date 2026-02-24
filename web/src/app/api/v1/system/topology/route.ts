import { NextResponse } from "next/server"
import { fetchWithBackendFailover, shardBackendUrls } from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

export async function GET() {
  const candidates = shardBackendUrls("/v1/system/topology")

  try {
    const { response, backend, attempts } = await fetchWithBackendFailover("/v1/system/topology")
    const data = await response.json()
    return NextResponse.json({ ...data, backend, backend_attempts: attempts }, { status: response.status })
  } catch (error) {
    return NextResponse.json(
      {
        status: "degraded",
        error: "Failed to get topology",
        backend_candidates: candidates,
        details: String(error),
      },
      { status: 502 }
    )
  }
}
