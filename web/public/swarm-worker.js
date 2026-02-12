/* global self, importScripts */

/**
 * Shard Swarm Worker — Service Worker for background P2P coordination.
 *
 * Responsibilities:
 * - js-libp2p browser node initialization
 * - Scout runtime coordination (double-dip lock, heartbeat)
 * - Work message handling from gossipsub
 * - WebLLM inference delegation (posts back to main thread for GPU work)
 * - Draft result publishing to the network
 *
 * NOTE: WebGPU is NOT available in service workers. Actual inference
 * must happen in the main thread or a dedicated web worker. This SW
 * handles coordination and posts INFERENCE_REQUEST to the page.
 */

// Import js-libp2p and related modules from CDN
// These are loaded dynamically when P2P is initialized
let libp2pModules = null;

let heartbeatTimer = null;
let doubleDipLocked = false;
let knownOracle = null;
let scoutReady = false;
let p2pNode = null;
let workTopic = null;
let resultTopic = null;
let isP2PInitialized = false;

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

// ─── P2P Initialization ─────────────────────────────────────────────────────

async function initP2P(bootstrapAddrs) {
  if (isP2PInitialized) {
    console.log("[swarm-worker] P2P already initialized");
    return;
  }

  try {
    // Dynamically import js-libp2p modules
    // In production, these would be bundled with the worker
    if (!libp2pModules) {
      // For now, we load from esm.sh CDN - in production, bundle these
      const { createLibp2p } = await import("https://esm.sh/libp2p@1.0.0");
      const { webSockets } = await import("https://esm.sh/@libp2p/websockets@8.0.0");
      const { gossipsub } = await import("https://esm.sh/@chainsafe/libp2p-gossipsub@13.0.0");
      const { noise } = await import("https://esm.sh/@chainsafe/libp2p-noise@15.0.0");
      const { yamux } = await import("https://esm.sh/@chainsafe/libp2p-yamux@6.0.0");
      const { bootstrap } = await import("https://esm.sh/@libp2p/bootstrap@10.0.0");

      libp2pModules = {
        createLibp2p,
        webSockets,
        gossipsub,
        noise,
        yamux,
        bootstrap,
      };
    }

    const { createLibp2p, webSockets, gossipsub, noise, yamux, bootstrap } = libp2pModules;

    // Configure bootstrap peers from topology
    const bootstrapList = bootstrapAddrs?.length > 0
      ? bootstrapAddrs
      : ["ws://127.0.0.1:4101"]; // Default fallback

    // Create libp2p node
    p2pNode = await createLibp2p({
      transports: [webSockets()],
      connectionEncryption: [noise()],
      streamMuxers: [yamux()],
      peerDiscovery: [
        bootstrap({
          list: bootstrapList,
          interval: 5000,
        }),
      ],
      services: {
        pubsub: gossipsub({
          emitSelf: false,
          gossipIncoming: true,
          fallbackToFloodsub: true,
        }),
      },
    });

    // Subscribe to topics
    workTopic = "shard-work";
    resultTopic = "shard-work-result";

    await p2pNode.services.pubsub.subscribe(workTopic);
    await p2pNode.services.pubsub.subscribe(resultTopic);

    // Handle incoming messages
    p2pNode.services.pubsub.addEventListener("message", (event) => {
      const { detail: message } = event;
      
      if (message.topic === workTopic) {
        try {
          const decoded = new TextDecoder().decode(message.data);
          const workMsg = JSON.parse(decoded);
          console.log("[swarm-worker] received work:", workMsg.request_id);
          onShardWork(workMsg);
        } catch (err) {
          console.warn("[swarm-worker] failed to decode work message:", err);
        }
      }
    });

    // Start the node
    await p2pNode.start();

    isP2PInitialized = true;

    console.log("[swarm-worker] P2P initialized, peer ID:", p2pNode.peerId.toString());

    // Notify page
    broadcast({
      type: "P2P_READY",
      peerId: p2pNode.peerId.toString(),
      listening: true,
    });

    // Log connected peers periodically
    setInterval(() => {
      const peers = p2pNode.getPeers();
      console.log("[swarm-worker] connected peers:", peers.length);
      broadcast({
        type: "P2P_PEERS",
        count: peers.length,
      });
    }, 30000);

  } catch (err) {
    console.error("[swarm-worker] P2P initialization failed:", err);
    broadcast({
      type: "P2P_ERROR",
      error: err.message,
    });
  }
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
  // Publish result to gossipsub if P2P is initialized
  if (isP2PInitialized && p2pNode && resultTopic) {
    try {
      const resultMsg = {
        request_id: result.request_id,
        peer_id: p2pNode.peerId.toString(),
        draft_tokens: result.draft_tokens || [],
        latency_ms: result.latency_ms || 0,
      };

      await p2pNode.services.pubsub.publish(
        resultTopic,
        new TextEncoder().encode(JSON.stringify(resultMsg))
      );

      console.log("[swarm-worker] published result to gossipsub:", result.request_id);
    } catch (err) {
      console.warn("[swarm-worker] failed to publish to gossipsub:", err);
    }
  }

  // Fallback: Forward to the Oracle API HTTP endpoint
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
    console.log("[swarm-worker] forwarded inference result via HTTP:", response.status);
  } catch (err) {
    console.warn("[swarm-worker] failed to forward result via HTTP:", err);
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
      // Initialize P2P if bootstrap addresses provided
      if (event.data.bootstrapAddrs || event.data.knownOracleAddr) {
        const addrs = event.data.bootstrapAddrs || [event.data.knownOracleAddr];
        initP2P(addrs);
      }
      break;

    case "INIT_P2P":
      initP2P(event.data.bootstrapAddrs);
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
