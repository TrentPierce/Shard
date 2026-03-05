export const runtime = 'edge';
import { NextResponse } from "next/server"
import { fetchWithBackendFailover, shardBackendUrls } from "@/lib/server/shard-backend"
import { deriveHealthState, healthStateToLegacyStatus } from "@/lib/server/health-state"
import { getChatProxySliSnapshot } from "@/lib/server/proxy-chat-sli"

export const dynamic = "force-dynamic"

type MetricsSnapshot = {
  payload: Record<string, unknown>
  updated_at_ms: number
}

type HealthSummary = {
  status?: string
  readiness_reason?: string
  ready_for_inference?: boolean
  connected_peers?: number
  active_browser_sessions?: number
  active_scouts?: number
  verified_peers?: number
  model_id?: string
  rust_version?: string
}

type BackendNodeProbe = {
  backend: string
  healthy: boolean
  status: string
  readiness_reason: string | null
  model_id: string | null
  rust_version: string | null
}

const MAX_STALE_METRICS_MS = 2 * 60 * 1000
let lastMetricsSnapshot: MetricsSnapshot | null = null

function updateMetricsSnapshot(payload: any): void {
  if (!payload || typeof payload !== "object") return
  if (
    typeof payload.tokens_processed_total !== "number" &&
    typeof payload.tokens_offloaded_to_scouts_total !== "number"
  ) {
    return
  }
  lastMetricsSnapshot = {
    payload: payload as Record<string, unknown>,
    updated_at_ms: Date.now(),
  }
}

function readFreshMetricsSnapshot(): MetricsSnapshot | null {
  if (!lastMetricsSnapshot) return null
  const ageMs = Date.now() - lastMetricsSnapshot.updated_at_ms
  return ageMs <= MAX_STALE_METRICS_MS ? lastMetricsSnapshot : null
}

function toNonNegativeInt(value: unknown): number {
  const n = Number(value)
  if (!Number.isFinite(n)) return 0
  return Math.max(0, Math.floor(n))
}

function backendBase(url: string): string {
  try {
    const parsed = new URL(url)
    return `${parsed.protocol}//${parsed.host}`
  } catch {
    return ""
  }
}

async function fetchJsonWithTimeout<T>(url: string, timeoutMs: number): Promise<T | null> {
  try {
    const response = await fetch(url, {
      method: "GET",
      cache: "no-store",
      signal: AbortSignal.timeout(timeoutMs),
    })
    if (!response.ok) return null
    return (await response.json()) as T
  } catch {
    return null
  }
}

async function probeBackendHealth(backendUrl: string): Promise<BackendNodeProbe | null> {
  const health = await fetchJsonWithTimeout<HealthSummary>(backendUrl, 3_000)
  if (!health) return null
  const status = String(health.status ?? "unknown").toLowerCase()
  const readinessReason =
    typeof health.readiness_reason === "string" && health.readiness_reason.trim().length > 0
      ? health.readiness_reason.trim()
      : null
  const healthy =
    (status === "ok" || status === "ready") &&
    health.ready_for_inference !== false

  return {
    backend: backendBase(backendUrl),
    healthy,
    status,
    readiness_reason: readinessReason,
    model_id: typeof health.model_id === "string" ? health.model_id : null,
    rust_version: typeof health.rust_version === "string" ? health.rust_version : null,
  }
}

export async function deriveActiveNodeEstimate(
  backendUrl: string,
  reportedActiveNodes: number,
): Promise<{
  activeNodes: number
  activeNodesReported: number
  activeNodesEstimated: number
  reachableNodes: number
  healthyNodes: number
  unhealthyNodes: number
  nodeProbes: BackendNodeProbe[]
  activeNodesEstimateSource:
    | "reported"
    | "backend_health_probe"
    | "verified_peers_plus_self"
}> {
  const base = backendBase(backendUrl)
  const backendCandidates = shardBackendUrls("/health")
  const nodeProbes = (
    await Promise.all(backendCandidates.map((candidate) => probeBackendHealth(candidate)))
  ).filter((probe): probe is BackendNodeProbe => probe !== null)
  const reachableNodes = nodeProbes.length
  const healthyNodes = nodeProbes.filter((probe) => probe.healthy).length
  const unhealthyNodes = Math.max(0, reachableNodes - healthyNodes)

  if (!base && healthyNodes === 0) {
    return {
      activeNodes: reportedActiveNodes,
      activeNodesReported: reportedActiveNodes,
      activeNodesEstimated: reportedActiveNodes,
      reachableNodes,
      healthyNodes: 0,
      unhealthyNodes,
      nodeProbes,
      activeNodesEstimateSource: "reported",
    }
  }

  const health = await fetchJsonWithTimeout<HealthSummary>(`${base}/health`, 2500)
  const verifiedPeers = toNonNegativeInt(health?.verified_peers)

  // Derive node count from direct backend probes first. Fall back to daemon-reported
  // verifier peer counts only when we cannot probe enough backends directly.
  const estimatedFromBackendHealth = reachableNodes
  const estimatedFromVerifiedPeers = Math.max(1, 1 + verifiedPeers)
  const estimatedNodes = Math.max(
    estimatedFromBackendHealth,
    estimatedFromVerifiedPeers,
    reportedActiveNodes,
  )
  const activeNodes = Math.max(reportedActiveNodes, estimatedNodes)

  let source: "reported" | "backend_health_probe" | "verified_peers_plus_self" = "reported"
  if (activeNodes > reportedActiveNodes) {
    source =
      estimatedFromBackendHealth >= estimatedFromVerifiedPeers
        ? "backend_health_probe"
        : "verified_peers_plus_self"
  }

  return {
    activeNodes,
    activeNodesReported: reportedActiveNodes,
    activeNodesEstimated: estimatedNodes,
    reachableNodes,
    healthyNodes,
    unhealthyNodes,
    nodeProbes,
    activeNodesEstimateSource: source,
  }
}

export async function GET() {
  // The Rust daemon exposes metrics at `/metrics/summary` (no `/v1` prefix).
  // We proxy that here for the web app and dashboards.
  const candidates = shardBackendUrls("/metrics/summary")
  const chatProxySli = getChatProxySliSnapshot()

  try {
    const { response, backend, attempts } = await fetchWithBackendFailover("/metrics/summary", {
      timeoutMs: 6_000,
      failoverOnStatuses: [500, 502, 503, 504, 521, 530],
    })
    const data = await response.json().catch(() => ({} as Record<string, unknown>))
    const reportedActiveNodes = toNonNegativeInt((data as any)?.active_nodes)
    const activeNodeEstimate = await deriveActiveNodeEstimate(backend, reportedActiveNodes)
    const mergedData = {
      ...data,
      active_nodes: activeNodeEstimate.activeNodes,
      active_nodes_reported: activeNodeEstimate.activeNodesReported,
      active_nodes_estimated: activeNodeEstimate.activeNodesEstimated,
      active_nodes_estimate_source: activeNodeEstimate.activeNodesEstimateSource,
      healthy_nodes: Math.max(
        toNonNegativeInt((data as any)?.healthy_nodes),
        activeNodeEstimate.healthyNodes,
      ),
      unhealthy_nodes: Math.max(
        toNonNegativeInt((data as any)?.unhealthy_nodes),
        activeNodeEstimate.unhealthyNodes,
      ),
      node_probes: activeNodeEstimate.nodeProbes,
    }
    updateMetricsSnapshot(mergedData)
    const healthState = deriveHealthState(data, response.ok)
    if (!response.ok) {
      const cached = readFreshMetricsSnapshot()
      if (cached) {
        return NextResponse.json(
          {
            ...cached.payload,
            status: healthStateToLegacyStatus(healthState),
            health_state: healthState,
            stale_snapshot: true,
            stale_snapshot_age_ms: Date.now() - cached.updated_at_ms,
            backend_status: response.status,
            backend,
            backend_attempts: attempts,
            proxy_chat_sli: chatProxySli,
          },
          { status: 200 },
        )
      }
      return NextResponse.json(
        {
          status: healthStateToLegacyStatus(healthState),
          health_state: healthState,
          backend_status: response.status,
          backend,
          backend_attempts: attempts,
          proxy_chat_sli: chatProxySli,
          ...data,
        },
        { status: 200 },
      )
    }
    return NextResponse.json(
      {
        ...mergedData,
        status: (mergedData as any)?.status ?? healthStateToLegacyStatus(healthState),
        health_state: healthState,
        backend,
        backend_attempts: attempts,
        proxy_chat_sli: chatProxySli,
      },
      { status: 200 },
    )
  } catch (error) {
    const cached = readFreshMetricsSnapshot()
    if (cached) {
      return NextResponse.json(
        {
          ...cached.payload,
          status: "degraded",
          health_state: "degraded",
          stale_snapshot: true,
          stale_snapshot_age_ms: Date.now() - cached.updated_at_ms,
          backend_candidates: candidates,
          error: String(error),
          proxy_chat_sli: chatProxySli,
        },
        { status: 200 },
      )
    }
    return NextResponse.json(
      {
        status: "unavailable",
        health_state: "unavailable",
        backend_candidates: candidates,
        error: String(error),
        proxy_chat_sli: chatProxySli,
      },
      { status: 200 },
    )
  }
}

