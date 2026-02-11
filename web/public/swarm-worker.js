/* global self */

/**
 * Shard Swarm Worker — Service Worker for background coordination.
 *
 * Responsibilities:
 * - Scout runtime coordination (double-dip lock, heartbeat)
 * - Work message routing between the page and swarm
 * - WebLLM inference delegation (posts back to main thread for GPU work)
 *
 * NOTE: WebGPU is NOT available in service workers. Actual inference
 * must happen in the main thread or a dedicated web worker. This SW
 * handles coordination and posts INFERENCE_REQUEST to the page.
 */

let heartbeatTimer = null;
let doubleDipLocked = false;
let knownOracle = null;
let scoutReady = false;

// ─── Install & Activate ─────────────────────────────────────────────────────

self.addEventListener("install", () => {
  self.skipWaiting();
});

self.addEventListener("activate", (event) => {
  event.waitUntil(self.clients.claim());
});

// ─── Messaging ──────────────────────────────────────────────────────────────

async function broadcast(msg) {
  const clients = await self.clients.matchAll({ type: "window" });
  clients.forEach((client) => client.postMessage(msg));
}

// ─── Scout Runtime ──────────────────────────────────────────────────────────

async function initScoutRuntime(oracleAddr, hasLocalOracle) {
  doubleDipLocked = Boolean(hasLocalOracle);
  knownOracle = oracleAddr;
  scoutReady = !doubleDipLocked;

  console.log("[swarm-worker] scout runtime initialized", {
    knownOracle,
    doubleDipLocked,
    scoutReady,
  });

  // Start heartbeat broadcast to keep the SW alive and inform the page
  if (heartbeatTimer) clearInterval(heartbeatTimer);
  heartbeatTimer = setInterval(() => {
    broadcast({
      type: "SW_HEARTBEAT",
      ts: Date.now(),
      scoutReady,
      doubleDipLocked,
      knownOracle,
    });
  }, 15000);

  // Notify page that worker is ready
  broadcast({
    type: "SW_READY",
    scoutReady,
    doubleDipLocked,
  });
}

// ─── Work Handling ──────────────────────────────────────────────────────────

async function onShardWork(msg) {
  if (doubleDipLocked) {
    console.log("[swarm-worker] ignoring work — double-dip locked");
    return;
  }

  if (!scoutReady) {
    console.log("[swarm-worker] ignoring work — scout not ready");
    return;
  }

  // Delegate inference to main thread (WebGPU not available in SW)
  broadcast({
    type: "INFERENCE_REQUEST",
    payload: {
      request_id: msg.request_id,
      prompt_context: msg.prompt_context,
      min_tokens: msg.min_tokens || 5,
    },
  });
}

// ─── Inference Result (from main thread after WebLLM runs) ──────────────────

async function onInferenceResult(result) {
  // Forward to the Oracle API for gossipsub publishing
  try {
    const response = await fetch("http://127.0.0.1:9091/broadcast-work", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        request_id: result.request_id,
        prompt_context: "",
        min_tokens: 0,
      }),
    });
    console.log("[swarm-worker] forwarded inference result", response.status);
  } catch (err) {
    console.warn("[swarm-worker] failed to forward result:", err);
  }

  // Also notify the page
  broadcast({
    type: "SCOUT_WORK_RESULT",
    payload: result,
  });
}

// ─── Message Router ─────────────────────────────────────────────────────────

self.addEventListener("message", (event) => {
  const { type } = event.data || {};

  switch (type) {
    case "INIT_SCOUT":
      initScoutRuntime(event.data.knownOracleAddr, event.data.hasLocalOracle);
      break;

    case "SHARD_WORK":
      onShardWork(event.data.payload);
      break;

    case "INFERENCE_RESULT":
      onInferenceResult(event.data.payload);
      break;

    case "UPDATE_DOUBLE_DIP":
      doubleDipLocked = Boolean(event.data.locked);
      scoutReady = !doubleDipLocked;
      console.log("[swarm-worker] double-dip lock updated:", doubleDipLocked);
      break;

    default:
      break;
  }
});

// ─── Fetch Intercept (optional: cache static assets) ────────────────────────

self.addEventListener("fetch", (event) => {
  // Pass through all requests — no caching strategy for now
  return;
});
