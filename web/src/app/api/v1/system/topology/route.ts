import { NextResponse } from "next/server"
import { fetchWithBackendFailover, shardBackendUrls } from "@/lib/server/shard-backend"

export const dynamic = "force-dynamic"

export async function GET() {
  const candidates = shardBackendUrls("/v1/system/topology")
  const fallbackBootstrapPeers = String(process.env.NEXT_PUBLIC_BOOTSTRAP_PEERS ?? "")
    .split(/[,\n ]+/)
    .map((s) => s.trim())
    .filter(Boolean)

  try {
    const { response, backend, attempts } = await fetchWithBackendFailover("/v1/system/topology", {
      timeoutMs: 30_000,
      failoverOnStatuses: [500, 502, 503, 504],
    })
    const data = await response.json()
    if (!response.ok) {
      return NextResponse.json(
        {
          status: "degraded",
          source: "proxy-non-ok",
          shard_webrtc_multiaddr: null,
          shard_quic_multiaddr: null,
          shard_ws_multiaddr: null,
          listen_addrs: [],
          bootstrap_peers: fallbackBootstrapPeers,
          known_peer_count: 0,
          public_api: false,
          backend_status: response.status,
          backend,
          backend_attempts: attempts,
          ...data,
        },
        { status: 200 },
      )
    }
    return NextResponse.json({ ...data, backend, backend_attempts: attempts }, { status: 200 })
  } catch (error) {
    return NextResponse.json(
      {
        status: "degraded",
        source: "proxy-fallback",
        shard_webrtc_multiaddr: null,
        shard_quic_multiaddr: null,
        shard_ws_multiaddr: null,
        listen_addrs: [],
        bootstrap_peers: fallbackBootstrapPeers,
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
