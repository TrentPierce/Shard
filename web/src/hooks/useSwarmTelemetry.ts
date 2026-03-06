"use client"

import { useEffect, useMemo, useRef, useState } from "react"
import { canUseLocalDaemonFallback, getPreferredLocalDaemonBase, localDaemonUrl } from "@/lib/runtime"

type ContributorHealth = "healthy" | "degraded" | "unknown"

type Contributor = {
  id: string
  role: "Scout" | "Shard"
  tokensProcessed: number
  efficiency: number
  health: ContributorHealth
  queueDepth: number
  latencyMs: number
  backend: string | null
  modelId: string | null
  rustVersion: string | null
  uptimeSeconds: number | null
  readinessReason: string | null
  label: string
  source: "metrics_node" | "peer_store" | "local_daemon" | "browser_scout"
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
  healthyShardCount: number
  throughputHistory: ThroughputSample[]
  contributors: Contributor[]
  totalTokensGenerated: number
}

function toPositiveNumber(value: unknown): number | null {
  const parsed = Number(value)
  if (!Number.isFinite(parsed)) return null
  return parsed >= 0 ? parsed : null
}

function compactId(value: unknown, fallback: string): string {
  const raw = String(value ?? "").trim()
  if (!raw) return fallback
  return raw.length > 18 ? raw.slice(0, 18) : raw
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
    const preferredBase = await getPreferredLocalDaemonBase()
    if (preferredBase) {
    ;[localHealth, localPeersData, localTopo, localMetrics] = await Promise.all([
      fetchJson(localDaemonUrl("/health", preferredBase)),
      fetchJson(localDaemonUrl("/v1/system/peers", preferredBase)),
      fetchJson(localDaemonUrl("/v1/system/topology", preferredBase)),
      fetchJson(localDaemonUrl("/metrics/summary", preferredBase)),
    ])
    }
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
  const activeScoutsFromMetrics = Number(metrics?.active_scout_nodes ?? metrics?.draft_capable_scouts ?? 0) || 0
  const activeBrowserSessionsFromMetrics = Number(metrics?.active_browser_sessions ?? 0) || 0
  const capacity = health?.capacity ?? topo?.capacity ?? 100
  const load = health?.load ?? topo?.load ?? 0
  const rustConnected = health?.rust_sidecar === "connected"
  const healthOk = health?.status === "ok"
  const bitnetLoaded = health?.bitnet_loaded === true

  // Prefer backend-probed verifier counts from metrics; fall back to the local daemon only
  // when we have no better network-wide signal.
  const healthyShardCount = Math.max(
    0,
    Number(metrics?.healthy_nodes ?? 0),
    Number(metrics?.active_nodes_reported ?? 0),
  )
  const localShardOnline = healthOk || rustConnected || topo?.status === "ok"
  const reportedShardCount = Math.max(
    0,
    Number(health?.shard_count ?? 0),
    Number(topo?.shard_count ?? 0),
    Number(metrics?.active_nodes ?? 0),
  )
  const inferredShardCount = healthyShardCount > 0 ? healthyShardCount : localShardOnline ? 1 : 0
  const shardCount = Math.max(reportedShardCount, inferredShardCount)

  // Show browser scout presence from explicit metrics first, then health snapshots.
  const scoutCount = Math.max(
    0,
    activeScoutsFromMetrics,
    activeBrowserSessionsFromMetrics,
    activeScoutsFromHealth,
    activeBrowserSessionsFromHealth,
  )

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

  const contributors: Contributor[] = []
  const seenContributorIds = new Set<string>()
  const registerContributor = (contributor: Contributor) => {
    if (seenContributorIds.has(contributor.id)) return
    seenContributorIds.add(contributor.id)
    contributors.push(contributor)
  }

  const nodeSummaries = Array.isArray(metrics?.nodes) ? metrics.nodes : []
  const nodeProbes = Array.isArray(metrics?.node_probes) ? metrics.node_probes : []

  nodeSummaries.forEach((node: any, index: number) => {
    const probe = nodeProbes[index] ?? null
    const queueDepth = toPositiveNumber(node?.queue_depth) ?? 0
    const latencyMs = toPositiveNumber(node?.node_latency_ms) ?? 0
    const uptimeSeconds = toPositiveNumber(node?.uptime_seconds)
    const healthy = Boolean(node?.healthy ?? probe?.healthy ?? false)
    registerContributor({
      id: compactId(node?.node_pubkey ?? probe?.backend, `node-${index + 1}`),
      role: "Shard",
      tokensProcessed: toPositiveNumber(node?.tokens_processed) ?? (index === 0 ? tokensProcessedTotal : 0),
      efficiency: healthy ? 95 : 68,
      health: healthy ? "healthy" : probe ? "degraded" : "unknown",
      queueDepth,
      latencyMs,
      backend: typeof probe?.backend === "string" ? probe.backend : null,
      modelId: typeof probe?.model_id === "string" ? probe.model_id : typeof health?.model_id === "string" ? health.model_id : null,
      rustVersion: typeof probe?.rust_version === "string" ? probe.rust_version : typeof health?.rust_version === "string" ? health.rust_version : null,
      uptimeSeconds,
      readinessReason: typeof probe?.readiness_reason === "string" ? probe.readiness_reason : null,
      label: node?.role === "gateway" ? "Gateway verifier" : "Verifier node",
      source: "metrics_node",
    })
  })

  const localPeerId = topo?.shard_peer_id ?? health?.peer_id
  if (shardCount > 0 && localPeerId) {
    registerContributor({
      id: compactId(localPeerId, "local-shard"),
      role: "Shard",
      tokensProcessed: tokensProcessedTotal,
      efficiency: bitnetLoaded ? 95 : 50,
      health: healthOk ? "healthy" : "degraded",
      queueDepth: toPositiveNumber(metrics?.queue_depth) ?? 0,
      latencyMs: toPositiveNumber(health?.latency_ms) ?? 0,
      backend: typeof health?.backend === "string" ? health.backend : null,
      modelId: typeof health?.model_id === "string" ? health.model_id : null,
      rustVersion: typeof health?.rust_version === "string" ? health.rust_version : null,
      uptimeSeconds: toPositiveNumber(health?.uptime_ms) != null ? Math.round((toPositiveNumber(health?.uptime_ms) ?? 0) / 1000) : null,
      readinessReason: typeof health?.readiness_reason === "string" ? health.readiness_reason : null,
      label: "Local verifier",
      source: "local_daemon",
    })
  }

  if (peersData?.peers && Array.isArray(peersData.peers)) {
    for (const peer of peersData.peers) {
      registerContributor({
        id: compactId(peer.peer_id || peer.id || "unknown", "peer"),
        role: "Shard",
        tokensProcessed: toPositiveNumber(peer.tokens_processed) ?? 0,
        efficiency: peer.verified ? 90 : 70,
        health: peer.verified ? "healthy" : "unknown",
        queueDepth: 0,
        latencyMs: 0,
        backend: null,
        modelId: null,
        rustVersion: null,
        uptimeSeconds: null,
        readinessReason: null,
        label: peer.verified ? "Connected verifier" : "Connected peer",
        source: "peer_store",
      })
    }
  }

  if (scoutCount > 0 || tokensOffloadedToScoutsTotal > 0) {
    registerContributor({
      id: `scout-pool-${scoutCount || 1}`,
      role: "Scout",
      tokensProcessed: tokensOffloadedToScoutsTotal,
      efficiency: scoutCount > 0 ? Math.max(55, Math.min(98, 72 + scoutCount)) : 60,
      health: scoutCount > 0 ? "healthy" : "unknown",
      queueDepth: 0,
      latencyMs: 0,
      backend: null,
      modelId: typeof health?.model_id === "string" ? health.model_id : null,
      rustVersion: null,
      uptimeSeconds: null,
      readinessReason: null,
      label: scoutCount > 0 ? `Browser scout pool (${scoutCount})` : "Browser scout pool",
      source: "browser_scout",
    })
  }

  return {
    globalTflops,
    scoutCount,
    shardCount,
    healthyShardCount,
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
    healthyShardCount: 0,
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
          healthyShardCount: data.healthyShardCount,
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
