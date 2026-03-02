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
 * - Double-dip prevention (local Shard detection)
 */

import { createLibp2p } from 'libp2p';
import { webSockets } from '@libp2p/websockets';
import { noise } from '@chainsafe/libp2p-noise';
import { yamux } from '@chainsafe/libp2p-yamux';
import { mplex } from '@libp2p/mplex';
import { identify } from '@libp2p/identify';
import { gossipsub as createGossipSub } from '@chainsafe/libp2p-gossipsub';
import { bootstrap } from '@libp2p/bootstrap';
import type { Libp2p } from 'libp2p';
import type { GossipSub } from '@chainsafe/libp2p-gossipsub';
import type { Message } from '@libp2p/interface';
import { multiaddr as createMultiaddr } from '@multiformats/multiaddr';

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
  from: string | undefined;
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
let peerCountTimer: ReturnType<typeof setInterval> | null = null;
let reconnectInFlight = false;
let workHandler: WorkHandler | null = null;
let resultHandler: ResultHandler | null = null;
let bootstrapPeersSnapshot: string[] = [];

const WORK_TOPIC = 'shard-work';
const RESULT_TOPIC = 'shard-work-result';
const DEFAULT_RECONNECT_INTERVAL_MS = 10000;
const PERSISTED_BOOTSTRAP_KEY = 'shard:p2p:bootstrap:v1';
const MAX_PERSISTED_BOOTSTRAPS = 32;
const RECONNECT_TICK_JITTER_MS = 1200;
const RECONNECT_DIAL_TIMEOUT_MS = 5000;

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
    // Prefer explicit topology-derived bootstrap peers; fallback only for local dev.
    const envBootstrapPeers = String(process.env.NEXT_PUBLIC_BOOTSTRAP_PEERS ?? '')
      .split(/[,\n ]+/)
      .map((v) => v.trim())
      .filter(Boolean);
    const isHttpsPage = typeof window !== 'undefined' && window.location.protocol === 'https:';
    const fallbackLocalPeers = isHttpsPage ? [] : ['/ip4/127.0.0.1/tcp/4101/ws'];
    const requestedPeers = mergeBootstrapPeers(
      config.bootstrapPeers && config.bootstrapPeers.length > 0
        ? config.bootstrapPeers
        : [...envBootstrapPeers, ...fallbackLocalPeers],
      readPersistedBootstrapPeers()
    );
    const bootstrapPeers = sanitizeBootstrapPeers(requestedPeers);
    bootstrapPeersSnapshot = bootstrapPeers;
    persistBootstrapPeers(bootstrapPeers);
    console.log('[p2p] Initializing with bootstrap peers:', bootstrapPeers);

    // Create libp2p node
    p2pNode = await createLibp2p({
      // Transports
      transports: [
        webSockets(),
      ],

      // Connection encryption
      connectionEncryption: [noise()],

      // Stream multiplexers
      streamMuxers: [
        yamux(),
        mplex(),
      ],

      // Peer discovery
      peerDiscovery: bootstrapPeers.length > 0
        ? [
          bootstrap({
            list: bootstrapPeers,
          }),
        ]
        : [],

      // Services
      services: {
        identify: identify(),
        pubsub: createGossipSub({
          emitSelf: config.emitSelf ?? false,
          fallbackToFloodsub: true,
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

    p2pNode.addEventListener('peer:connect', (event: any) => {
      const remoteAddr = extractRemoteAddr(event);
      if (remoteAddr && isWsDialablePeer(remoteAddr)) {
        persistBootstrapPeers([remoteAddr]);
      }
      console.log('[p2p] Peer connected:', event?.detail?.toString?.() ?? 'unknown');
    });

    p2pNode.addEventListener('peer:disconnect', (event: any) => {
      const remoteAddr = extractRemoteAddr(event);
      console.warn('[p2p] Peer disconnected:', event?.detail?.toString?.() ?? 'unknown', remoteAddr ?? '');
    });

    // Log connection status periodically
    if (peerCountTimer) {
      clearInterval(peerCountTimer);
    }
    peerCountTimer = setInterval(() => {
      if (!p2pNode) return;
      const peerCount = p2pNode.getPeers().length;
      console.log('[p2p] Connected peers:', peerCount);
    }, 30000);

    const reconnectInterval = config.reconnectInterval ?? DEFAULT_RECONNECT_INTERVAL_MS;
    if (reconnectTimer) {
      clearInterval(reconnectTimer);
    }
    reconnectTimer = setInterval(() => {
      const jitterMs = Math.floor(Math.random() * RECONNECT_TICK_JITTER_MS);
      setTimeout(() => {
        void reconnectBootstrapPeers();
      }, jitterMs);
    }, reconnectInterval);
    void reconnectBootstrapPeers();

    return p2pNode.peerId.toString();
  } catch (error) {
    console.error('[p2p] Initialization failed:', error);
    throw new Error(`P2P initialization failed: ${error instanceof Error ? error.message : String(error)}`);
  }
}

function isWsDialablePeer(peer: string): boolean {
  const normalized = peer.toLowerCase();
  return (
    normalized.startsWith('ws://') ||
    normalized.startsWith('wss://') ||
    normalized.includes('/ws/') ||
    normalized.endsWith('/ws') ||
    normalized.includes('/wss/') ||
    normalized.endsWith('/wss')
  );
}

function isSecureWsPeer(peer: string): boolean {
  const normalized = peer.toLowerCase();
  return normalized.startsWith('wss://') || normalized.includes('/wss/') || normalized.endsWith('/wss');
}

function sanitizeBootstrapPeers(peers: string[]): string[] {
  const isHttpsPage = typeof window !== 'undefined' && window.location.protocol === 'https:';
  const unique = new Set<string>();

  for (const raw of peers) {
    const peer = String(raw || '').trim();
    if (!peer) continue;
    if (!isWsDialablePeer(peer)) continue;
    if (isHttpsPage && !isSecureWsPeer(peer)) continue;
    unique.add(peer);
  }

  return Array.from(unique);
}

function mergeBootstrapPeers(primary: string[], secondary: string[]): string[] {
  const out = new Set<string>();
  for (const peer of [...primary, ...secondary]) {
    const trimmed = String(peer || '').trim();
    if (trimmed.length === 0) continue;
    out.add(trimmed);
  }
  return Array.from(out);
}

function readPersistedBootstrapPeers(): string[] {
  if (typeof window === 'undefined') {
    return [];
  }
  try {
    const raw = window.localStorage.getItem(PERSISTED_BOOTSTRAP_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.map((v) => String(v)).filter(Boolean);
  } catch {
    return [];
  }
}

function persistBootstrapPeers(peers: string[]): void {
  if (typeof window === 'undefined') {
    return;
  }
  try {
    const merged = mergeBootstrapPeers(readPersistedBootstrapPeers(), peers).slice(
      0,
      MAX_PERSISTED_BOOTSTRAPS
    );
    window.localStorage.setItem(PERSISTED_BOOTSTRAP_KEY, JSON.stringify(merged));
  } catch {
    // best-effort persistence
  }
}

function extractRemoteAddr(event: any): string | null {
  const addr = event?.detail?.remoteAddr ?? event?.detail?.connection?.remoteAddr;
  if (!addr) return null;
  try {
    return typeof addr === 'string' ? addr : addr.toString();
  } catch {
    return null;
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function shuffleArray<T>(input: T[]): T[] {
  const copy = [...input];
  for (let i = copy.length - 1; i > 0; i -= 1) {
    const j = Math.floor(Math.random() * (i + 1));
    [copy[i], copy[j]] = [copy[j], copy[i]];
  }
  return copy;
}

async function dialWithTimeout(node: Libp2p, peer: string, timeoutMs: number): Promise<void> {
  const ma = createMultiaddr(peer);
  await Promise.race([
    node.dial(ma as any) as Promise<unknown>,
    (async () => {
      await sleep(timeoutMs);
      throw new Error(`dial_timeout_${timeoutMs}ms`);
    })(),
  ]);
}

async function reconnectBootstrapPeers(): Promise<void> {
  if (!p2pNode) return;
  if (reconnectInFlight) return;
  reconnectInFlight = true;
  // Maintain at least MIN_MESH_PEERS connections for redundancy.
  // Previously stopped reconnecting at 1 peer, which left the node
  // vulnerable to single-peer failure.
  try {
    const MIN_MESH_PEERS = 2;
    if (p2pNode.getPeers().length >= MIN_MESH_PEERS) return;

    const reconnectCandidates = sanitizeBootstrapPeers(
      mergeBootstrapPeers(bootstrapPeersSnapshot, readPersistedBootstrapPeers())
    );
    if (reconnectCandidates.length === 0) {
      return;
    }

    const MAX_RECONNECT_DIALS_PER_TICK = 3;
    for (const peer of shuffleArray(reconnectCandidates).slice(0, MAX_RECONNECT_DIALS_PER_TICK)) {
      try {
        await dialWithTimeout(p2pNode, peer, RECONNECT_DIAL_TIMEOUT_MS);
        console.log('[p2p] Reconnected bootstrap peer:', peer);
        persistBootstrapPeers([peer]);
      } catch (error) {
        console.warn('[p2p] Reconnect attempt failed:', peer, error);
      }
    }
  } finally {
    reconnectInFlight = false;
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
  if (peerCountTimer) {
    clearInterval(peerCountTimer);
    peerCountTimer = null;
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
  reconnectInFlight = false;
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
      from: 'from' in message ? message.from?.toString() : undefined,
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
