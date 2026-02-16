"use client"

import { useEffect, useMemo, useRef, useState } from "react"
import {
  createInitialTelemetry,
  type SwarmTelemetrySnapshot,
} from "@/lib/mockSwarmTelemetry"

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

export function useSwarmTelemetry() {
  const [telemetry, setTelemetry] = useState<SwarmTelemetrySnapshot>(() => createInitialTelemetry())
  const [isConnected, setIsConnected] = useState(false)
  const reconnectAttempt = useRef(0)
  const reconnectTimer = useRef<number | null>(null)

  useEffect(() => {
    let socket: WebSocket | null = null
    let isUnmounted = false

    const clearTimer = () => {
      if (reconnectTimer.current !== null) {
        window.clearTimeout(reconnectTimer.current)
        reconnectTimer.current = null
      }
    }

    const connect = () => {
      if (isUnmounted) {
        return
      }

      socket = new WebSocket(resolveTelemetryWsUrl())

      socket.onopen = () => {
        reconnectAttempt.current = 0
        setIsConnected(true)
      }

      socket.onmessage = (event) => {
        try {
          const parsed = JSON.parse(event.data) as IncomingTelemetryMessage
          const now = new Date()
          const sample = {
            timestamp: now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
            tflops: Number(parsed.global_tflops.toFixed(2)),
          }

          setTelemetry((current) => ({
            ...current,
            globalTflops: sample.tflops,
            scoutCount: parsed.active_scouts,
            shardCount: Math.max(0, parsed.connected_peers - parsed.active_scouts),
            throughputHistory: [...current.throughputHistory.slice(-(MAX_HISTORY - 1)), sample],
          }))
        } catch {
          // Ignore malformed telemetry packets.
        }
      }

      socket.onclose = () => {
        setIsConnected(false)
        if (isUnmounted) {
          return
        }

        reconnectAttempt.current += 1
        const exponentialBackoff = Math.min(
          MAX_RECONNECT_DELAY_MS,
          INITIAL_RECONNECT_DELAY_MS * 2 ** Math.min(reconnectAttempt.current, MAX_EXP_BACKOFF_STEP),
        )
        const jitter = exponentialBackoff * (Math.random() * 0.3 - 0.15)
        const backoffMs = Math.max(INITIAL_RECONNECT_DELAY_MS, Math.round(exponentialBackoff + jitter))
        reconnectTimer.current = window.setTimeout(connect, backoffMs)
      }

      socket.onerror = () => {
        socket?.close()
      }
    }

    connect()

    return () => {
      isUnmounted = true
      setIsConnected(false)
      clearTimer()
      socket?.close()
    }
  }, [])

  const statusLabel = useMemo(
    () => (isConnected ? "LIVE WS STREAM" : "RECONNECTING..."),
    [isConnected],
  )

  return { telemetry, isConnected, statusLabel }
}
