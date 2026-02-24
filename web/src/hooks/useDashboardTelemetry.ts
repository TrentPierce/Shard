"use client"

import { useEffect, useMemo, useRef, useState } from "react"
import { useSwarmTelemetry } from "@/hooks/useSwarmTelemetry"
import { createInitialTelemetry, tickTelemetry } from "@/lib/mockSwarmTelemetry"

type DashboardTelemetry = {
  verifierNodes: number
  scouts: number
  tokensPerSecond: number
  totalTokensGenerated: number
  isLive: boolean
}

export function useDashboardTelemetry(): DashboardTelemetry {
  const { telemetry, isConnected } = useSwarmTelemetry()
  const [fallback, setFallback] = useState(() => createInitialTelemetry())
  const liveTotalRef = useRef(0)

  useEffect(() => {
    const timer = setInterval(() => {
      setFallback((current) => tickTelemetry(current))
    }, 2500)

    return () => clearInterval(timer)
  }, [])

  const tokensPerSecond = useMemo(() => {
    if (telemetry.throughputHistory.length > 1) {
      const latest = telemetry.throughputHistory[telemetry.throughputHistory.length - 1]
      return Math.max(45, Math.round(latest.tflops * 35))
    }
    return Math.round(fallback.globalTflops * 35)
  }, [telemetry.throughputHistory, fallback.globalTflops])

  const totalTokensGenerated = useMemo(() => {
    const liveTotal = telemetry.contributors.reduce((acc, item) => acc + item.tokensProcessed, 0)

    if (liveTotal > 0) {
      liveTotalRef.current = liveTotal
      return liveTotal
    }

    const fallbackTotal = fallback.contributors.reduce((acc, item) => acc + item.tokensProcessed, 0)
    const base = liveTotalRef.current > 0 ? liveTotalRef.current : fallbackTotal

    return base + tokensPerSecond * 8
  }, [telemetry.contributors, fallback.contributors, tokensPerSecond])

  return {
    verifierNodes: telemetry.shardCount > 0 ? telemetry.shardCount : fallback.shardCount,
    scouts: telemetry.scoutCount > 0 ? telemetry.scoutCount : fallback.scoutCount,
    tokensPerSecond,
    totalTokensGenerated,
    isLive: isConnected,
  }
}
