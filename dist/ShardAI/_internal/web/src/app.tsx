"use client"

import { useEffect, useState } from "react"
import { fetchTopology, heartbeatOracle, initSwarmWorker, probeLocalOracle } from "./swarm"

type Mode = "loading" | "local-oracle" | "scout"

export default function App() {
  const [mode, setMode] = useState<Mode>("loading")
  const [keepAliveState, setKeepAliveState] = useState("disconnected")
  const [heartbeatState, setHeartbeatState] = useState("idle")
  const [knownOracleAddr, setKnownOracleAddr] = useState("")

  useEffect(() => {
    const socket = new WebSocket("ws://127.0.0.1:8081/keepalive")
    socket.onopen = () => setKeepAliveState("connected")
    socket.onerror = () => setKeepAliveState("error")
    socket.onclose = () => setKeepAliveState("closed")

    const onWorkerMsg = (event: MessageEvent) => {
      if (event.data?.type === "SW_HEARTBEAT") {
        setKeepAliveState(`worker-heartbeat@${new Date(event.data.ts).toLocaleTimeString()}`)
      }
    }
    navigator.serviceWorker?.addEventListener("message", onWorkerMsg)

    const boot = async () => {
      const probe = await probeLocalOracle()
      if (probe.available) {
        setMode("local-oracle")
        return
      }

      const topo = await fetchTopology()
      setKnownOracleAddr(topo.oracle_webrtc_multiaddr ?? "")
      await initSwarmWorker(topo.oracle_webrtc_multiaddr)
      setMode("scout")
    }

    void boot()

    return () => {
      socket.close()
      navigator.serviceWorker?.removeEventListener("message", onWorkerMsg)
    }
  }, [])

  const runHeartbeat = async () => {
    if (!knownOracleAddr) {
      setHeartbeatState("missing topology oracle_webrtc_multiaddr")
      return
    }

    setHeartbeatState("dialing")
    const result = await heartbeatOracle(knownOracleAddr)
    if (result.ok) {
      setHeartbeatState(`PONG rtt=${result.rttMs?.toFixed(1)}ms`)
      return
    }
    setHeartbeatState(`failed: ${result.detail}`)
  }

  return (
    <main>
      <h1>Shard Web Client</h1>
      {mode === "loading" && <p>Bootstrapping swarm...</p>}
      {mode === "local-oracle" && <p>Local Oracle detected. WebGPU Scout disabled.</p>}
      {mode === "scout" && <p>Scout mode enabled. Service Worker connected to swarm.</p>}

      <p>Status Window Keep-Alive: {keepAliveState}</p>
      <p>Known Oracle Multiaddr: {knownOracleAddr || "(waiting for /v1/system/topology)"}</p>
      <p>Heartbeat: {heartbeatState}</p>
      <button onClick={runHeartbeat}>Send PING</button>
    </main>
  )
}
