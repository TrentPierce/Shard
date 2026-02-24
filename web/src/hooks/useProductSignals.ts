"use client"

import { useEffect, useMemo, useState } from "react"
import { apiUrl } from "@/lib/config"
import {
  CONTRIBUTION_STATUS_EVENT,
  type ContributionStatus,
} from "@/lib/contribution-status"

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
  contributionTransitions: number
  lastContributionState: string
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
      contributionTransitions: 0,
      lastContributionState: "unknown",
    }
  }
  try {
    const parsed = JSON.parse(localStorage.getItem(ANALYTICS_KEY) || "{}")
    return {
      sessions: Number(parsed.sessions || 0),
      successfulChats: Number(parsed.successfulChats || 0),
      failedChats: Number(parsed.failedChats || 0),
      avgLatencyMs: Number(parsed.avgLatencyMs || 0),
      contributionTransitions: Number(parsed.contributionTransitions || 0),
      lastContributionState: String(parsed.lastContributionState || "unknown"),
    }
  } catch {
    return {
      sessions: 0,
      successfulChats: 0,
      failedChats: 0,
      avgLatencyMs: 0,
      contributionTransitions: 0,
      lastContributionState: "unknown",
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
    const onSuccess = (event: Event) => {
      const detail = (event as CustomEvent<{ latencyMs?: number }>).detail
      const latencyMs = Number(detail?.latencyMs || 0)
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
        }
        saveAnalytics(next)
        return next
      })
    }

    const onFailure = () => {
      setAnalytics((prev) => {
        const next = { ...prev, failedChats: prev.failedChats + 1 }
        saveAnalytics(next)
        return next
      })
    }

    window.addEventListener("shard:chat-success", onSuccess as EventListener)
    window.addEventListener("shard:chat-failure", onFailure)
    return () => {
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
