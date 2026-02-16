"use client"

import { useEffect, useMemo, useRef, useState } from "react"
import {
  createInitialTelemetry,
  type SwarmTelemetrySnapshot,
} from "@/lib/mockSwarmTelemetry"
import { apiUrl } from "@/lib/config"

type IncomingTelemetryMessage = {
  connected_peers: number
  active_scouts: number
  global_tflops: number
}

const LOCAL_TELEMETRY_WS_URL = "ws://127.0.0.1:9093/telemetry/ws"
const TELEMETRY_PATH = "/telemetry/ws"
const INITIAL_RECONNECT_DELAY_MS = 1_500
const MAX_RECONNECT_DELAY_MS = 30_000
const MAX_EXP_BACKOFF_STEP = 6
const MAX_HISTORY = 18

function resolveTelemetryWsUrl() {
  const configured = process.env.NEXT_PUBLIC_WS_URL?.trim()

  if (!configured) {
    return LOCAL_TELEMETRY_WS_URL
  }

  try {
    const parsed = new URL(configured)
    if (parsed.pathname === "/" || parsed.pathname === "") {
      parsed.pathname = TELEMETRY_PATH
    }
    return parsed.toString()
  } catch {
    return configured
  }
}

async function fetchTelemetryFromApi(): Promise<SwarmTelemetrySnapshot> {
  try {
    // Fetch health to get peer count
    const healthRes = await fetch(apiUrl("/health"))
    const health = await healthRes.json()
    
    // Fetch peers to count
    const peersRes = await fetch(apiUrl("/v1/system/peers"))
    const peers = await peersRes.json()
    
    const connectedPeers = peers?.peers?.length || 0
    const globalTflops = health?.capacity ? (health.capacity * connectedPeers * 0.1) : 0
    
    return {
      globalTflops: Math.round(globalTflops * 100) / 100,
      scoutCount: connectedPeers, // Estimate all peers as scouts
      shardCount: 1, // Local shard
      throughputHistory: [],
      contributors: []
    }
  } catch (e) {
    console.error("Failed to fetch telemetry from API:", e)
    return createInitialTelemetry()
  }
}

export function useSwarmTelemetry() {
  const [telemetry, setTelemetry] = useState<SwarmTelemetrySnapshot>(() => createInitialTelemetry())
  const [isConnected, setIsConnected] = useState(false)
  const reconnectAttempt = useRef(0)
  const reconnectTimer = useRef<number | null>(null)

  // Poll API for telemetry instead of WebSocket (more reliable for now)
  useEffect(() => {
    let isUnmounted = false

    const pollTelemetry = async () => {
      try {
        const data = await fetchTelemetryFromApi()
        if (!isUnmounted) {
          setTelemetry(current => ({
            ...current,
            ...data,
            // Keep existing throughput history and contributors if API returns empty
            throughputHistory: data.throughputHistory.length > 0 
              ? data.throughputHistory 
              : current.throughputHistory,
            contributors: data.contributors.length > 0 
              ? data.contributors 
              : current.contributors,
          }))
          setIsConnected(true)
        }
      } catch (e) {
        console.error("Telemetry poll failed:", e)
        if (!isUnmounted) {
          setIsConnected(false)
        }
      }
    }

    // Initial fetch
    pollTelemetry()

    // Poll every 5 seconds
    const interval = setInterval(pollTelemetry, 5000)

    return () => {
      isUnmounted = true
      clearInterval(interval)
    }
  }, [])

  const statusLabel = useMemo(
    () => (isConnected ? "LIVE API POLL" : "CONNECTING..."),
    [isConnected],
  )

  return { telemetry, isConnected, statusLabel }
}
