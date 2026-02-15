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

const WS_URL = process.env.NEXT_PUBLIC_TELEMETRY_WS_URL ?? "ws://127.0.0.1:9093/telemetry/ws"
const MAX_HISTORY = 18

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

      socket = new WebSocket(WS_URL)

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
        const backoffMs = Math.min(10_000, 1_000 * 2 ** Math.min(reconnectAttempt.current, 4))
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
