/**
 * Shard P2P Client - js-libp2p Browser Implementation
 *
 * This module provides a browser-compatible libp2p client that connects to
 * the Rust daemon's gossipsub network via WebSocket transport.
 *
 * Key Features:
 * - WebSocket transport for browser compatibility
 * - Gossipsub pub/sub for work distribution
 * - Automatic reconnection to bootstrap peers
 * - Double-dip prevention (local Oracle detection)
 */

import { createLibp2p } from 'libp2p';
import { webSockets } from '@libp2p/websockets';
import { webTransport } from '@libp2p/webtransport';
import { noise } from '@chainsafe/libp2p-noise';
import { yamux } from '@chainsafe/libp2p-yamux';
import { mplex } from '@libp2p/mplex';
import { gossipsub } from '@chainsafe/libp2p-gossipsub';
import { bootstrap } from '@libp2p/bootstrap';
import type { Libp2p } from 'libp2p';
import type { GossipSub } from '@chainsafe/libp2p-gossipsub';

// ─── Types ──────────────────────────────────────────────────────────────────

export type P2PConfig = {
  /**
   * Bootstrap peer addresses (WebSocket multiaddrs)
   * Example: ["ws://127.0.0.1:4101", "/dns4/example.com/tcp/4101/wss/p2p/Qm..."]
   */
  bootstrapPeers?: string[];

  /**
   * Reconnection interval in milliseconds (default: 5000)
   */
  reconnectInterval?: number;

  /**
   * Whether to emit messages to self (default: false)
   */
  emitSelf?: boolean;
};

export type P2PMessage = {
  topic: string;
  data: Uint8Array;
  from: string;
};

export type WorkMessage = {
  request_id: string;
  prompt_context: string;
  min_tokens: number;
  timestamp?: number;
};

export type WorkResultMessage = {
  request_id: string;
  draft_tokens: string[];
  scout_id: string;
  timestamp: number;
};

export type WorkHandler = (work: WorkMessage) => void | Promise<void>;
export type ResultHandler = (result: WorkResultMessage) => void | Promise<void>;

// ─── State ──────────────────────────────────────────────────────────────────

let p2pNode: Libp2p | null = null;
let gossipsub: GossipSub | null = null;
let isInitialized = false;
let reconnectTimer: ReturnType<typeof setInterval> | null = null;
let workHandler: WorkHandler | null = null;
let resultHandler: ResultHandler | null = null;

const WORK_TOPIC = 'shard-work';
const RESULT_TOPIC = 'shard-work-result';

// ─── Core Functions ────────────────────────────────────────────────────────

/**
 * Initialize the libp2p browser node with WebSocket transport and Gossipsub.
 *
 * @param config - P2P configuration options
 * @returns Promise that resolves when the node is ready
 */
export async function initP2P(config: P2PConfig = {}): Promise<string> {
  if (isInitialized && p2pNode) {
    console.log('[p2p] Already initialized');
    return p2pNode.peerId.toString();
  }

  try {
    // Configure bootstrap peers
    const bootstrapPeers = config.bootstrapPeers || ['ws://127.0.0.1:4101'];
    const reconnectInterval = config.reconnectInterval || 5000;

    console.log('[p2p] Initializing with bootstrap peers:', bootstrapPeers);

    // Create libp2p node
    p2pNode = await createLibp2p({
      // Transports
      transports: [
        webSockets(),
        webTransport(),
      ],

      // Connection encryption
      connectionEncryption: [noise()],

      // Stream multiplexers
      streamMuxers: [
        yamux(),
        mplex(),
      ],

      // Peer discovery
      peerDiscovery: [
        bootstrap({
          list: bootstrapPeers,
          interval: reconnectInterval,
        }),
      ],

      // Services
      services: {
        pubsub: gossipsub({
          emitSelf: config.emitSelf ?? false,
          gossipIncoming: true,
          fallbackToFloodsub: true,
          allowPublishToZeroPeers: true,
        }),
      },
    });

    // Start the node
    await p2pNode.start();

    // Get reference to gossipsub service
    gossipsub = p2pNode.services.pubsub as GossipSub;

    // Subscribe to work topic
    await subscribeToTopic(WORK_TOPIC, (msg) => {
      if (msg.topic === WORK_TOPIC) {
        handleWorkMessage(msg);
      }
    });

    // Subscribe to result topic (for monitoring)
    await subscribeToTopic(RESULT_TOPIC, (msg) => {
      if (msg.topic === RESULT_TOPIC) {
        handleResultMessage(msg);
      }
    });

    isInitialized = true;

    console.log('[p2p] Initialized successfully');
    console.log('[p2p] Peer ID:', p2pNode.peerId.toString());
    console.log('[p2p] Multiaddrs:', p2pNode.getMultiaddrs().map(m => m.toString()));

    // Log connection status periodically
    setInterval(() => {
      if (!p2pNode) return;
      const peerCount = p2pNode.getPeers().length;
      console.log('[p2p] Connected peers:', peerCount);
    }, 30000);

    return p2pNode.peerId.toString();
  } catch (error) {
    console.error('[p2p] Initialization failed:', error);
    throw new Error(`P2P initialization failed: ${error instanceof Error ? error.message : String(error)}`);
  }
}

/**
 * Subscribe to work requests from the network.
 *
 * @param handler - Callback function to handle incoming work
 */
export function subscribeToWork(handler: WorkHandler): void {
  workHandler = handler;
  console.log('[p2p] Work handler registered');
}

/**
 * Subscribe to result messages (optional - for monitoring other scouts).
 *
 * @param handler - Callback function to handle incoming results
 */
export function subscribeToResults(handler: ResultHandler): void {
  resultHandler = handler;
  console.log('[p2p] Result handler registered');
}

/**
 * Publish draft results to the network.
 *
 * @param result - The work result to publish
 * @returns Promise that resolves when published
 */
export async function publishResult(result: WorkResultMessage): Promise<boolean> {
  if (!gossipsub || !p2pNode) {
    console.error('[p2p] Cannot publish: P2P not initialized');
    return false;
  }

  try {
    const message = JSON.stringify(result);
    const data = new TextEncoder().encode(message);

    await gossipsub.publish(RESULT_TOPIC, data);

    console.log('[p2p] Published result for request:', result.request_id);
    return true;
  } catch (error) {
    console.error('[p2p] Failed to publish result:', error);
    return false;
  }
}

/**
 * Get the current peer ID.
 *
 * @returns The peer ID string or null if not initialized
 */
export function getPeerId(): string | null {
  return p2pNode?.peerId.toString() || null;
}

/**
 * Get the number of connected peers.
 *
 * @returns The number of connected peers
 */
export function getPeerCount(): number {
  return p2pNode?.getPeers().length || 0;
}

/**
 * Check if the P2P node is initialized and ready.
 *
 * @returns True if initialized, false otherwise
 */
export function isReady(): boolean {
  return isInitialized && p2pNode !== null;
}

/**
 * Stop the P2P node and cleanup.
 *
 * @returns Promise that resolves when stopped
 */
export async function stopP2P(): Promise<void> {
  if (reconnectTimer) {
    clearInterval(reconnectTimer);
    reconnectTimer = null;
  }

  if (p2pNode) {
    try {
      await p2pNode.stop();
      console.log('[p2p] Stopped');
    } catch (error) {
      console.error('[p2p] Error stopping:', error);
    }
  }

  p2pNode = null;
  gossipsub = null;
  isInitialized = false;
}

// ─── Internal Helpers ──────────────────────────────────────────────────────

/**
 * Subscribe to a gossipsub topic.
 *
 * @param topic - The topic name
 * @param handler - Message handler callback
 */
async function subscribeToTopic(
  topic: string,
  handler: (msg: P2PMessage) => void
): Promise<void> {
  if (!gossipsub) {
    throw new Error('Gossipsub not initialized');
  }

  await gossipsub.subscribe(topic);
  console.log('[p2p] Subscribed to topic:', topic);

  // Listen for messages
  gossipsub.addEventListener('message', (event) => {
    const { detail: message } = event;

    // Create P2PMessage wrapper
    const p2pMessage: P2PMessage = {
      topic: message.topic,
      data: message.data,
      from: message.from.toString(),
    };

    handler(p2pMessage);
  });
}

/**
 * Handle incoming work messages.
 *
 * @param msg - The P2P message
 */
async function handleWorkMessage(msg: P2PMessage): Promise<void> {
  try {
    const decoded = new TextDecoder().decode(msg.data);
    const work: WorkMessage = JSON.parse(decoded);

    console.log('[p2p] Received work request:', work.request_id);

    if (workHandler) {
      await workHandler(work);
    }
  } catch (error) {
    console.error('[p2p] Failed to handle work message:', error);
  }
}

/**
 * Handle incoming result messages.
 *
 * @param msg - The P2P message
 */
async function handleResultMessage(msg: P2PMessage): Promise<void> {
  try {
    const decoded = new TextDecoder().decode(msg.data);
    const result: WorkResultMessage = JSON.parse(decoded);

    console.log('[p2p] Received result for:', result.request_id);

    if (resultHandler) {
      await resultHandler(result);
    }
  } catch (error) {
    console.error('[p2p] Failed to handle result message:', error);
  }
}

/**
 * Parse a WebSocket multiaddr to extract the host and port.
 *
 * @param multiaddr - The multiaddr string
 * @returns Object with host and port, or null if invalid
 */
export function parseWebSocketMultiaddr(multiaddr: string): { host: string; port: number } | null {
  try {
    // Simple parsing for ws://host:port and wss://host:port
    const wsMatch = multiaddr.match(/(?:ws|wss):\/\/([^:]+):(\d+)/);
    if (!wsMatch) return null;

    const [, host, portStr] = wsMatch;
    const port = parseInt(portStr, 10);

    return { host, port };
  } catch {
    return null;
  }
}
