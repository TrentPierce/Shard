/* global self */

let heartbeatTimer = null
let doubleDipLocked = false

self.addEventListener("install", () => {
  self.skipWaiting()
})

self.addEventListener("activate", (event) => {
  event.waitUntil(self.clients.claim())
})

async function sendWorkResult(result) {
  const clients = await self.clients.matchAll({ type: "window" })
  clients.forEach((client) => {
    client.postMessage({ type: "SCOUT_WORK_RESULT", payload: result })
  })
}

async function onShardWork(msg) {
  if (doubleDipLocked) {
    return
  }

  // Scaffold: WebLLM generation for max 5 tokens.
  const draftTokens = ["draft", "token", "placeholder"]
  await sendWorkResult({
    request_id: msg.request_id,
    peer_id: "scout-browser",
    draft_tokens: draftTokens.slice(0, Math.max(1, msg.min_tokens ?? 5)),
    latency_ms: 12.5,
  })
}

async function initScoutRuntime(knownOracleAddr, hasLocalOracle) {
  doubleDipLocked = Boolean(hasLocalOracle)
  console.log("[swarm-worker] scout runtime initialized", { knownOracleAddr, doubleDipLocked })

  // TODO: connect js-libp2p and subscribe to gossipsub topic `shard-work`.

  if (heartbeatTimer) {
    clearInterval(heartbeatTimer)
  }

  heartbeatTimer = setInterval(async () => {
    const clients = await self.clients.matchAll({ type: "window" })
    clients.forEach((client) => {
      client.postMessage({ type: "SW_HEARTBEAT", ts: Date.now() })
    })
  }, 15000)
}

self.addEventListener("message", (event) => {
  if (event.data?.type === "INIT_SCOUT") {
    void initScoutRuntime(event.data.knownOracleAddr, event.data.hasLocalOracle)
  }

  if (event.data?.type === "SHARD_WORK") {
    void onShardWork(event.data.payload)
  }
})
