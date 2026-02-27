"use client"

import { useEffect, useMemo, useRef, useState } from "react"
import { useSwarmTelemetry } from "@/hooks/useSwarmTelemetry"

type DashboardTelemetry = {
  verifierNodes: number
  scouts: number
  tokensPerSecond: number
  totalTokensGenerated: number
  isLive: boolean
  healthState: "ready" | "degraded" | "unavailable"
  statusLabel: "READY" | "DEGRADED" | "OFFLINE"
}

export function useDashboardTelemetry(): DashboardTelemetry {
  const { telemetry, isConnected, statusLabel } = useSwarmTelemetry()
  const liveTotalRef = useRef(0)
  const [lastKnownTps, setLastKnownTps] = useState(0)

  useEffect(() => {
    if (!isConnected) return
    if (telemetry.throughputHistory.length < 1) return
    const latest = telemetry.throughputHistory[telemetry.throughputHistory.length - 1]
    setLastKnownTps(Math.max(1, Math.round(latest.tflops * 35)))
  }, [isConnected, telemetry.throughputHistory])

  const tokensPerSecond = useMemo(() => {
    if (telemetry.throughputHistory.length > 0) {
      const latest = telemetry.throughputHistory[telemetry.throughputHistory.length - 1]
      return Math.max(1, Math.round(latest.tflops * 35))
    }
    return isConnected ? Math.max(1, Math.round(telemetry.globalTflops * 35)) : lastKnownTps
  }, [isConnected, telemetry.globalTflops, telemetry.throughputHistory, lastKnownTps])

  const totalTokensGenerated = useMemo(() => {
    const liveTotal = Math.max(
      telemetry.totalTokensGenerated,
      telemetry.contributors.reduce((acc, item) => acc + item.tokensProcessed, 0),
    )

    if (liveTotal > 0) {
      liveTotalRef.current = liveTotal
      return liveTotal
    }

    const base = liveTotalRef.current

    return base > 0 ? base + tokensPerSecond * 8 : 0
  }, [telemetry.contributors, telemetry.totalTokensGenerated, tokensPerSecond])

  return {
    verifierNodes: telemetry.shardCount,
    scouts: telemetry.scoutCount,
    tokensPerSecond,
    totalTokensGenerated,
    isLive: isConnected,
    healthState: telemetry.healthState,
    statusLabel,
  }
}
