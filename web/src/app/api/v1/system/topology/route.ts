import { NextResponse } from "next/server"
import { fetchWithBackendFailover, shardBackendUrls } from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

export async function GET() {
  const candidates = shardBackendUrls("/v1/system/topology")

  try {
    const { response, backend, attempts } = await fetchWithBackendFailover("/v1/system/topology", {
      timeoutMs: 30_000,
      failoverOnStatuses: [500, 502, 503, 504],
    })
    const data = await response.json()
    return NextResponse.json({ ...data, backend, backend_attempts: attempts }, { status: response.status })
  } catch (error) {
    return NextResponse.json(
      {
        status: "degraded",
        source: "proxy-fallback",
        shard_webrtc_multiaddr: null,
        shard_quic_multiaddr: null,
        shard_ws_multiaddr: null,
        listen_addrs: [],
        known_peer_count: 0,
        public_api: false,
        error: "Failed to get topology",
        backend_candidates: candidates,
        details: String(error),
      },
      { status: 200 }
    )
  }
}
