/* global self */

let heartbeatTimer = null
let isLocalOracle = false

self.addEventListener("install", () => {
  self.skipWaiting()
})

self.addEventListener("activate", (event) => {
  event.waitUntil(self.clients.claim())
})

async function detectLocalOracle() {
  try {
    const res = await fetch("http://localhost:8000/v1/system/topology")
    if (!res.ok) {
      isLocalOracle = false
      return
    }
    const topo = await res.json()
    isLocalOracle = Boolean(topo?.oracle_webrtc_multiaddr)
    if (isLocalOracle) {
      console.log("SHARD: Local Oracle detected. Disabling WebGPU Scout to save VRAM.")
    }
  } catch {
    isLocalOracle = false
  }
}

async function sendWorkResult(result) {
  const clients = await self.clients.matchAll({ type: "window" })
  clients.forEach((client) => {
    client.postMessage({ type: "SCOUT_WORK_RESULT", payload: result })
  })
}

async function onShardWork(msg) {
  if (isLocalOracle) {
    return
  }

  // Scaffold: WebLLM generation for max 5 tokens.
  const draftTokens = ["draft", "token", "placeholder", "guess", "batch"]
  await sendWorkResult({
    request_id: msg.request_id,
    sequence_id: msg.sequence_id ?? 0,
    peer_id: "scout-browser",
    draft_tokens: draftTokens.slice(0, Math.max(1, msg.min_tokens ?? 5)),
    latency_ms: 12.5,
  })
}

async function initScoutRuntime(knownOracleAddr) {
  await detectLocalOracle()
  console.log("[swarm-worker] scout runtime initialized", { knownOracleAddr, isLocalOracle })

  // TODO: connect js-libp2p and subscribe to gossipsub topic `shard-work`.

  if (heartbeatTimer) {
    clearInterval(heartbeatTimer)
  }

  heartbeatTimer = setInterval(async () => {
    const clients = await self.clients.matchAll({ type: "window" })
    clients.forEach((client) => {
      client.postMessage({ type: "SW_HEARTBEAT", ts: Date.now(), isLocalOracle })
    })
  }, 15000)
}

self.addEventListener("message", (event) => {
  if (event.data?.type === "INIT_SCOUT") {
    void initScoutRuntime(event.data.knownOracleAddr)
  }

  if (event.data?.type === "SHARD_WORK") {
    void onShardWork(event.data.payload)
  }
})
