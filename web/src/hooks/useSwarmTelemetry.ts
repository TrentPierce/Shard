"use client"

import { useEffect, useMemo, useRef, useState } from "react"
import { apiUrl } from "@/lib/config"

type Contributor = {
  id: string
  role: "Scout" | "Shard"
  tokensProcessed: number
  efficiency: number
}

type ThroughputSample = {
  timestamp: string
  tflops: number
}

type SwarmTelemetrySnapshot = {
  globalTflops: number
  scoutCount: number
  shardCount: number
  throughputHistory: ThroughputSample[]
  contributors: Contributor[]
}

async function fetchRealTelemetry(): Promise<SwarmTelemetrySnapshot> {
  // Fetch all three endpoints in parallel for speed
  const [healthRes, peersRes, topoRes] = await Promise.allSettled([
    fetch(apiUrl("/health")),
    fetch(apiUrl("/v1/system/peers")),
    fetch(apiUrl("/v1/system/topology")),
  ])

  // Parse each response, defaulting gracefully on failure
  const health =
    healthRes.status === "fulfilled" && healthRes.value.ok
      ? await healthRes.value.json().catch(() => null)
      : null

  const peersData =
    peersRes.status === "fulfilled" && peersRes.value.ok
      ? await peersRes.value.json().catch(() => null)
      : null

  const topo =
    topoRes.status === "fulfilled" && topoRes.value.ok
      ? await topoRes.value.json().catch(() => null)
      : null

  // If none of the endpoints responded, we're truly offline
  if (!health && !peersData && !topo) {
    throw new Error("All API endpoints unreachable")
  }

  const connectedPeers =
    Number(
      peersData?.peers?.length ??
      peersData?.count ??
      health?.active_scouts ??
      health?.connected_peers ??
      0
    ) || 0
  const capacity = health?.capacity ?? topo?.capacity ?? 100
  const load = health?.load ?? topo?.load ?? 0
  const rustConnected = health?.rust_sidecar === "connected"
  const bitnetLoaded = health?.bitnet_loaded === true

  // The EC2 Shard itself is always 1 active Shard node when healthy
  const shardCount = (rustConnected || topo?.status === "ok") ? 1 : 0

  // Scout count comes from connected P2P peers
  const scoutCount = Math.max(0, connectedPeers)

  // TFLOPs estimate: base capacity from the Shard + scout contributions
  // Even with 0 scouts, the Shard itself has compute capacity
  const baseTflops = shardCount > 0 ? capacity * 0.01 : 0
  const scoutTflops = scoutCount > 0
    ? (capacity * scoutCount * 0.1 * (1 - load / Math.max(capacity, 1)))
    : 0
  const globalTflops = Math.round((baseTflops + scoutTflops) * 100) / 100

  // Build contributor list: start with the Shard node itself
  const contributors: Contributor[] = []

  if (shardCount > 0 && topo?.shard_peer_id) {
    contributors.push({
      id: topo.shard_peer_id.slice(0, 16),
      role: "Shard",
      tokensProcessed: 0, // TODO: Real token tracking
      efficiency: bitnetLoaded ? 95 : 50,
    })
  }

  // Add connected peers as Scout contributors
  if (peersData?.peers && Array.isArray(peersData.peers)) {
    for (const peer of peersData.peers) {
      contributors.push({
        id: (peer.peer_id || peer.id || "unknown").slice(0, 16),
        role: "Scout",
        tokensProcessed: peer.tokens_processed ?? 0,
        efficiency: peer.verified ? 90 : 70,
      })
    }
  }

  return {
    globalTflops,
    scoutCount,
    shardCount,
    throughputHistory: [],
    contributors,
  }
}

export function useSwarmTelemetry() {
  const [telemetry, setTelemetry] = useState<SwarmTelemetrySnapshot>({
    globalTflops: 0,
    scoutCount: 0,
    shardCount: 0,
    throughputHistory: [],
    contributors: []
  })
  const [isConnected, setIsConnected] = useState(false)
  const historyRef = useRef<ThroughputSample[]>([])
  const contributorsRef = useRef<Contributor[]>([])

  // Poll API for real telemetry
  useEffect(() => {
    let isUnmounted = false

    const pollTelemetry = async () => {
      try {
        const data = await fetchRealTelemetry()
        if (isUnmounted) return

        // Update history (keep last 60 samples = ~3 minutes at 3s intervals)
        const now = new Date()
        const sample: ThroughputSample = {
          timestamp: now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" }),
          tflops: data.globalTflops
        }

        historyRef.current = [...historyRef.current.slice(-59), sample]
        contributorsRef.current = data.contributors.length > 0
          ? data.contributors
          : contributorsRef.current

        setTelemetry({
          globalTflops: data.globalTflops,
          scoutCount: data.scoutCount,
          shardCount: data.shardCount,
          throughputHistory: historyRef.current,
          contributors: contributorsRef.current
        })
        setIsConnected(true)
      } catch (e) {
        console.error("Telemetry poll failed:", e)
        if (!isUnmounted) {
          setIsConnected(false)
        }
      }
    }

    // Initial fetch
    pollTelemetry()

    // Poll every 3 seconds for live updates
    const interval = setInterval(pollTelemetry, 3000)

    return () => {
      isUnmounted = true
      clearInterval(interval)
    }
  }, [])

  const statusLabel = useMemo(
    () => (isConnected ? "LIVE" : "OFFLINE"),
    [isConnected],
  )

  return { telemetry, isConnected, statusLabel }
}
