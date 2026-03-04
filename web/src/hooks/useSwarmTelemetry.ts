"use client"

import { useEffect, useMemo, useRef, useState } from "react"
import { canUseLocalDaemonFallback, localDaemonUrl } from "@/lib/runtime"

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
  healthState: "ready" | "degraded" | "unavailable"
  globalTflops: number
  scoutCount: number
  shardCount: number
  throughputHistory: ThroughputSample[]
  contributors: Contributor[]
  totalTokensGenerated: number
}

async function fetchRealTelemetry(): Promise<SwarmTelemetrySnapshot> {
  const fetchJson = async (url: string): Promise<any | null> => {
    try {
      const res = await fetch(url, { cache: "no-store" })
      if (!res.ok) return null
      return await res.json()
    } catch {
      return null
    }
  }

  const [proxyHealth, proxyPeersData, proxyTopo, proxyMetrics] = await Promise.all([
    fetchJson("/api/health"),
    fetchJson("/api/v1/system/peers"),
    fetchJson("/api/v1/system/topology"),
    fetchJson("/api/v1/metrics/summary"),
  ])

  let localHealth: any | null = null
  let localPeersData: any | null = null
  let localTopo: any | null = null
  let localMetrics: any | null = null

  if (canUseLocalDaemonFallback()) {
    ;[localHealth, localPeersData, localTopo, localMetrics] = await Promise.all([
      fetchJson(localDaemonUrl("/health")),
      fetchJson(localDaemonUrl("/v1/system/peers")),
      fetchJson(localDaemonUrl("/v1/system/topology")),
      fetchJson(localDaemonUrl("/metrics/summary")),
    ])
  }

  if (
    !proxyHealth &&
    !proxyPeersData &&
    !proxyTopo &&
    !proxyMetrics &&
    !localHealth &&
    !localPeersData &&
    !localTopo &&
    !localMetrics
  ) {
    throw new Error("All API endpoints unreachable")
  }

  // Prefer local daemon data when explicitly available and safe.
  const health = localHealth ?? proxyHealth
  const peersData = localPeersData ?? proxyPeersData
  const topo = localTopo ?? proxyTopo
  const metrics = localMetrics ?? proxyMetrics
  const rawHealthState = String(health?.health_state ?? health?.status ?? "").toLowerCase()
  const healthState: "ready" | "degraded" | "unavailable" =
    rawHealthState === "ready" || rawHealthState === "ok"
      ? "ready"
      : rawHealthState === "unavailable"
      ? "unavailable"
      : "degraded"

  // Get peer count - handle both {peers: [...]} and {count: N} formats
  const peersList = peersData?.peers ?? []
  const peerCountFromPeersEndpoint = Array.isArray(peersList)
    ? peersList.length
    : Number(peersData?.count ?? 0)
  const connectedPeersFromHealth = Number(health?.connected_peers ?? 0) || 0
  const connectedPeers = Math.max(0, peerCountFromPeersEndpoint || 0, connectedPeersFromHealth)
  
  const activeScoutsFromHealth = Number(health?.active_scouts ?? 0) || 0
  const activeBrowserSessionsFromHealth = Number(health?.active_browser_sessions ?? 0) || 0
  const capacity = health?.capacity ?? topo?.capacity ?? 100
  const load = health?.load ?? topo?.load ?? 0
  const rustConnected = health?.rust_sidecar === "connected"
  const healthOk = health?.status === "ok"
  const bitnetLoaded = health?.bitnet_loaded === true

  // Count shards conservatively from all available sources.
  const localShardOnline = healthOk || rustConnected || topo?.status === "ok"
  const reportedShardCount = Math.max(
    0,
    Number(health?.shard_count ?? 0),
    Number(topo?.shard_count ?? 0),
  )
  // Do not infer verifier nodes from connected peers: browser scouts are peers too.
  const inferredShardCount = localShardOnline ? 1 : 0
  const shardCount = Math.max(reportedShardCount, inferredShardCount)

  // Show browser scout presence from either explicit active scouts or active browser sessions.
  const scoutCount = Math.max(0, activeScoutsFromHealth, activeBrowserSessionsFromHealth)

  // TFLOPs estimate: base capacity from the Shard + scout contributions
  // Even with 0 scouts, the Shard itself has compute capacity
  const baseTflops = shardCount > 0 ? capacity * 0.01 : 0
  const scoutTflops = scoutCount > 0
    ? (capacity * scoutCount * 0.1 * (1 - load / Math.max(capacity, 1)))
    : 0
  const globalTflops = Math.round((baseTflops + scoutTflops) * 100) / 100
  const tokensProcessedTotal = Math.max(0, Number(metrics?.tokens_processed_total ?? 0) || 0)
  const tokensOffloadedToScoutsTotal = Math.max(
    0,
    Number(metrics?.tokens_offloaded_to_scouts_total ?? 0) || 0,
  )
  const totalTokensGenerated = tokensProcessedTotal + tokensOffloadedToScoutsTotal

  // Build contributor list: start with the Shard node itself
  const contributors: Contributor[] = []

  const localPeerId = topo?.shard_peer_id ?? health?.peer_id
  if (shardCount > 0 && localPeerId) {
    contributors.push({
      id: localPeerId.slice(0, 16),
      role: "Shard",
      tokensProcessed: tokensProcessedTotal,
      efficiency: bitnetLoaded ? 95 : 50,
    })
  }

  // Add connected peers as Shard contributors
  if (peersData?.peers && Array.isArray(peersData.peers)) {
    for (const peer of peersData.peers) {
      contributors.push({
        id: (peer.peer_id || peer.id || "unknown").slice(0, 16),
        role: "Shard",
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
    totalTokensGenerated,
    healthState,
  }
}

export function useSwarmTelemetry() {
  const [telemetry, setTelemetry] = useState<SwarmTelemetrySnapshot>({
    healthState: "unavailable",
    globalTflops: 0,
    scoutCount: 0,
    shardCount: 0,
    throughputHistory: [],
    contributors: [],
    totalTokensGenerated: 0,
  })
  const [isConnected, setIsConnected] = useState(false)
  const [isLoading, setIsLoading] = useState(true)
  const [errorMessage, setErrorMessage] = useState<string | null>(null)
  const [consecutiveFailures, setConsecutiveFailures] = useState(0)
  const historyRef = useRef<ThroughputSample[]>([])
  const contributorsRef = useRef<Contributor[]>([])
  const pollRef = useRef<() => Promise<void>>()

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
          healthState: data.healthState,
          globalTflops: data.globalTflops,
          scoutCount: data.scoutCount,
          shardCount: data.shardCount,
          throughputHistory: historyRef.current,
          contributors: contributorsRef.current,
          totalTokensGenerated: data.totalTokensGenerated,
        })
        setIsConnected(data.healthState !== "unavailable")
        setErrorMessage(null)
        setConsecutiveFailures(0)
      } catch (e) {
        console.error("Telemetry poll failed:", e)
        if (!isUnmounted) {
          setIsConnected(false)
          setConsecutiveFailures((value) => {
            const next = value + 1
            if (next >= 3) {
              setTelemetry((prev) => ({
                ...prev,
                healthState: "unavailable",
              }))
            }
            return next
          })
          setErrorMessage(e instanceof Error ? e.message : "Telemetry unavailable")
        }
      } finally {
        if (!isUnmounted) {
          setIsLoading(false)
        }
      }
    }
    pollRef.current = pollTelemetry

    // Initial fetch
    pollTelemetry()

    // Safety timeout to ensure loading state doesn't hang UI
    const safetyTimeout = setTimeout(() => {
      if (!isUnmounted) setIsLoading(false)
    }, 5000)

    // Poll every 3 seconds for live updates
    const interval = setInterval(pollTelemetry, 3000)

    return () => {
      isUnmounted = true
      clearInterval(interval)
      clearTimeout(safetyTimeout)
    }
  }, [])

  const statusLabel: "READY" | "DEGRADED" | "OFFLINE" = useMemo(
    () => {
      if (telemetry.healthState === "ready") return "READY"
      if (telemetry.healthState === "degraded") return "DEGRADED"
      return "OFFLINE"
    },
    [telemetry.healthState],
  )

  const retryNow = () => {
    pollRef.current?.()
  }

  return { telemetry, isConnected, isLoading, statusLabel, errorMessage, retryNow }
}
