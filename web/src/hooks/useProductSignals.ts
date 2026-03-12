"use client"

import { useEffect, useMemo, useState } from "react"
import { apiUrl } from "@/lib/config"
import {
  CONTRIBUTION_STATUS_EVENT,
  type ContributionStatus,
} from "@/lib/contribution-status"
import {
  DEFAULT_ROUTE_ANALYTICS,
  reduceRouteAnalytics,
  ROUTE_DECISION_EVENT,
  type RouteAnalyticsSnapshot,
  type RouteDecisionDetail,
} from "@/lib/route-telemetry"

type HealthSnapshot = {
  rust_sidecar?: string
  rust_uptime_ms?: number
  rust_version?: string
  connected_peers?: number
  active_scouts?: number
  bitnet_loaded?: boolean
  last_incident?: string
}

type AnalyticsSnapshot = {
  sessions: number
  successfulChats: number
  failedChats: number
  avgLatencyMs: number
  browserLocalSuccessfulChats: number
  networkSuccessfulChats: number
  avgBrowserLatencyMs: number
  avgNetworkLatencyMs: number
  lastChatTransport: string
  contributionTransitions: number
  lastContributionState: string
  routeDecisions: number
  localRouteDecisions: number
  networkRouteDecisions: number
  compactedRouteDecisions: number
  browserFallbackRoutes: number
  avgRouteComplexityX100: number
  lastRouteKind: string
  lastRouteReason: string
}

const ANALYTICS_KEY = "shard-analytics-v1"
const SESSION_MARKER = "shard-session-active"

function loadAnalytics(): AnalyticsSnapshot {
  if (typeof window === "undefined") {
    return {
      sessions: 0,
      successfulChats: 0,
      failedChats: 0,
      avgLatencyMs: 0,
      browserLocalSuccessfulChats: 0,
      networkSuccessfulChats: 0,
      avgBrowserLatencyMs: 0,
      avgNetworkLatencyMs: 0,
      lastChatTransport: "unknown",
      contributionTransitions: 0,
      lastContributionState: "unknown",
      ...DEFAULT_ROUTE_ANALYTICS,
    }
  }
  try {
    const parsed = JSON.parse(localStorage.getItem(ANALYTICS_KEY) || "{}")
    return {
      sessions: Number(parsed.sessions || 0),
      successfulChats: Number(parsed.successfulChats || 0),
      failedChats: Number(parsed.failedChats || 0),
      avgLatencyMs: Number(parsed.avgLatencyMs || 0),
      browserLocalSuccessfulChats: Number(parsed.browserLocalSuccessfulChats || 0),
      networkSuccessfulChats: Number(parsed.networkSuccessfulChats || 0),
      avgBrowserLatencyMs: Number(parsed.avgBrowserLatencyMs || 0),
      avgNetworkLatencyMs: Number(parsed.avgNetworkLatencyMs || 0),
      lastChatTransport: String(parsed.lastChatTransport || "unknown"),
      contributionTransitions: Number(parsed.contributionTransitions || 0),
      lastContributionState: String(parsed.lastContributionState || "unknown"),
      routeDecisions: Number(parsed.routeDecisions || 0),
      localRouteDecisions: Number(parsed.localRouteDecisions || 0),
      networkRouteDecisions: Number(parsed.networkRouteDecisions || 0),
      compactedRouteDecisions: Number(parsed.compactedRouteDecisions || 0),
      browserFallbackRoutes: Number(parsed.browserFallbackRoutes || 0),
      avgRouteComplexityX100: Number(parsed.avgRouteComplexityX100 || 0),
      lastRouteKind: String(parsed.lastRouteKind || "none"),
      lastRouteReason: String(parsed.lastRouteReason || "unknown"),
    }
  } catch {
    return {
      sessions: 0,
      successfulChats: 0,
      failedChats: 0,
      avgLatencyMs: 0,
      browserLocalSuccessfulChats: 0,
      networkSuccessfulChats: 0,
      avgBrowserLatencyMs: 0,
      avgNetworkLatencyMs: 0,
      lastChatTransport: "unknown",
      contributionTransitions: 0,
      lastContributionState: "unknown",
      ...DEFAULT_ROUTE_ANALYTICS,
    }
  }
}

function saveAnalytics(next: AnalyticsSnapshot) {
  if (typeof window === "undefined") return
  localStorage.setItem(ANALYTICS_KEY, JSON.stringify(next))
}

export function useProductSignals() {
  const [health, setHealth] = useState<HealthSnapshot>({})
  const [analytics, setAnalytics] = useState<AnalyticsSnapshot>(() => loadAnalytics())

  useEffect(() => {
    if (typeof window === "undefined") return
    if (!sessionStorage.getItem(SESSION_MARKER)) {
      sessionStorage.setItem(SESSION_MARKER, "1")
      setAnalytics((prev) => {
        const next = { ...prev, sessions: prev.sessions + 1 }
        saveAnalytics(next)
        return next
      })
    }
  }, [])

  useEffect(() => {
    const onContributionStatus = (event: Event) => {
      const detail = (event as CustomEvent<ContributionStatus>).detail
      if (!detail?.state) return
      setAnalytics((prev) => {
        const next = {
          ...prev,
          contributionTransitions: prev.contributionTransitions + 1,
          lastContributionState: detail.state,
        }
        saveAnalytics(next)
        return next
      })
    }

    window.addEventListener(CONTRIBUTION_STATUS_EVENT, onContributionStatus as EventListener)
    return () => {
      window.removeEventListener(CONTRIBUTION_STATUS_EVENT, onContributionStatus as EventListener)
    }
  }, [])

  useEffect(() => {
    const onRouteDecision = (event: Event) => {
      const detail = (event as CustomEvent<RouteDecisionDetail>).detail
      if (!detail?.decision) return
      setAnalytics((prev) => {
        const routeSnapshot = reduceRouteAnalytics(
          {
            routeDecisions: prev.routeDecisions,
            localRouteDecisions: prev.localRouteDecisions,
            networkRouteDecisions: prev.networkRouteDecisions,
            compactedRouteDecisions: prev.compactedRouteDecisions,
            browserFallbackRoutes: prev.browserFallbackRoutes,
            avgRouteComplexityX100: prev.avgRouteComplexityX100,
            lastRouteKind: (prev.lastRouteKind as RouteAnalyticsSnapshot["lastRouteKind"]) ?? "none",
            lastRouteReason: prev.lastRouteReason,
          },
          detail,
        )
        const next = { ...prev, ...routeSnapshot }
        saveAnalytics(next)
        return next
      })
    }

    const onSuccess = (event: Event) => {
      const detail = (
        event as CustomEvent<{
          latencyMs?: number
          transport?: "browser_local" | "network_stream" | "network_sync"
        }>
      ).detail
      const latencyMs = Number(detail?.latencyMs || 0)
      const transport = detail?.transport || "unknown"
      setAnalytics((prev) => {
        const successfulChats = prev.successfulChats + 1
        const nextAvg =
          successfulChats > 0
            ? Math.round(
                ((prev.avgLatencyMs * prev.successfulChats) + latencyMs) / successfulChats
              )
            : prev.avgLatencyMs
        const next = {
          ...prev,
          successfulChats,
          avgLatencyMs: nextAvg,
          browserLocalSuccessfulChats:
            prev.browserLocalSuccessfulChats + (transport === "browser_local" ? 1 : 0),
          networkSuccessfulChats:
            prev.networkSuccessfulChats + (transport === "browser_local" ? 0 : 1),
          avgBrowserLatencyMs:
            transport === "browser_local"
              ? Math.round(
                  ((prev.avgBrowserLatencyMs * prev.browserLocalSuccessfulChats) + latencyMs) /
                    Math.max(prev.browserLocalSuccessfulChats + 1, 1),
                )
              : prev.avgBrowserLatencyMs,
          avgNetworkLatencyMs:
            transport === "browser_local"
              ? prev.avgNetworkLatencyMs
              : Math.round(
                  ((prev.avgNetworkLatencyMs * prev.networkSuccessfulChats) + latencyMs) /
                    Math.max(prev.networkSuccessfulChats + 1, 1),
                ),
          lastChatTransport: transport,
        }
        saveAnalytics(next)
        return next
      })
    }

    const onFailure = (event: Event) => {
      const detail = (
        event as CustomEvent<{
          transport?: "browser_local" | "network_stream" | "network_sync"
        }>
      ).detail
      setAnalytics((prev) => {
        const next = {
          ...prev,
          failedChats: prev.failedChats + 1,
          lastChatTransport: detail?.transport || prev.lastChatTransport,
        }
        saveAnalytics(next)
        return next
      })
    }

    window.addEventListener(ROUTE_DECISION_EVENT, onRouteDecision as EventListener)
    window.addEventListener("shard:chat-success", onSuccess as EventListener)
    window.addEventListener("shard:chat-failure", onFailure)
    return () => {
      window.removeEventListener(ROUTE_DECISION_EVENT, onRouteDecision as EventListener)
      window.removeEventListener("shard:chat-success", onSuccess as EventListener)
      window.removeEventListener("shard:chat-failure", onFailure)
    }
  }, [])

  useEffect(() => {
    let cancelled = false
    const loadHealth = async () => {
      try {
        const res = await fetch(apiUrl("/health"), { cache: "no-store" })
        if (!res.ok) return
        const data = (await res.json()) as HealthSnapshot
        if (!cancelled) setHealth(data)
      } catch {
        // ignore health polling errors
      }
    }
    loadHealth()
    const interval = setInterval(loadHealth, 15000)
    return () => {
      cancelled = true
      clearInterval(interval)
    }
  }, [])

  const successRate = useMemo(() => {
    const total = analytics.successfulChats + analytics.failedChats
    if (total === 0) return 100
    return Math.round((analytics.successfulChats / total) * 100)
  }, [analytics.failedChats, analytics.successfulChats])

  return { health, analytics, successRate }
}
