//! Shard Daemon — P2P networking sidecar for the Shard inference network.
//!
//! Provides:
//! - libp2p swarm with TCP, WebSocket transports (WebRTC-direct on Linux/Mac)
//! - Gossipsub topics: `shard-work`, `shard-work-result`
//! - Request/response protocols for handshake and draft verification
//! - Embedded HTTP control-plane API for the Python driver
//!
//! Build:   cargo build --release
//! Run:     ./shard-daemon --control-port 9091

#![allow(clippy::too_many_arguments)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::unnecessary_cast)]

use anyhow::Result;
use axum::body::Body;
use axum::middleware::{from_fn_with_state, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::{
    extract::Path as AxumPath,
    extract::Query,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State as AxumState,
    },
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use clap::{Args, Parser, Subcommand};
use libp2p::{
    autonat, dcutr,
    futures::StreamExt,
    gossipsub::{self, IdentTopic, MessageAuthenticity},
    identify,
    kad::{
        store::MemoryStore, Behaviour as KadBehaviour, Event as KadEvent, GetProvidersOk,
        QueryResult,
    },
    ping, relay,
    request_response::{self, OutboundRequestId, ProtocolSupport},
    swarm::{NetworkBehaviour, SwarmEvent},
    Multiaddr, PeerId, StreamProtocol,
};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use shard_common::types::{WorkRequest, WorkResponse};
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, Mutex, Notify};
use tower_http::cors::{Any, CorsLayer};

pub mod api;
pub mod bootstrap_discovery;
pub mod bootstrap_ring;
pub mod canary;
pub mod consensus;
pub mod inference;
pub mod ledger;
pub mod network;
pub mod scheduler;
pub mod security;
pub mod telemetry_ws;
use api::*;
use canary::*;
use consensus::leader::{ElectionMessage, LeaderElectionConfig, LeaderElectionHandle, LeaderInput};
use network::policy::{NetworkPolicy, PolicyDecision};
use scheduler::*;
use shard_common::common::node_config::{NodeRole, NodeRuntimeConfig};
use shard_common::common::pow_challenge::PowChallengeManager;
use shard_common::common::signed_envelope::{EnvelopeVerifier, SignedEnvelope};
use shard_common::mesh::race_router::{RaceKey, RaceRouter, RaceSubmitOutcome};
use shard_crypto::crypto::wallet_backup::{export_wallet, import_wallet, verify_backup};
use shard_crypto::identity::NodeIdentity;
use shard_gateway::gateway::fallback::{
    execute_centralized_fallback, ActiveRequestState, FallbackConfig,
};
use shard_gateway::gateway::validate_work_request;
use shard_ledger::ledger::state::{ComputeCreditTx, LedgerState};
use shard_ledger::ledger::store::LedgerStore;
use shard_ledger::ledger::sync::{hash_probe_segments, LedgerSyncRequest, LedgerSyncResponse};
use shard_metrics::metrics::alerts::{Alert, AlertManager};
use shard_metrics::metrics::cost::{estimate as estimate_cost, CostEstimateInput};
use shard_metrics::metrics::persistence::{MetricsPersistence, PersistedNodeMetricReport};
use shard_metrics::metrics::{
    NodeMetricReport, NodeMetricSnapshot, PrometheusSample, SystemMetrics,
};
use shard_network::network::layer_registry::{
    provider_key, LayerHostAnnouncement, LayerRoutingTable,
};
use shard_network::network::obfuscation::{deobfuscate_bytes, obfuscate_bytes, random_nonce};
use shard_network::network::private_mesh::{
    hash_api_key, PrivateMeshRegistry, PrivateRouteDecision,
};
use shard_network::network::tensor_wire::TensorWirePacket;
use shard_scheduler::scheduler::{
    load_reputation, save_reputation, weighted_select, NodeReputation, NodeSchedulerInput,
};

// ─── CLI ────────────────────────────────────────────────────────────────────

// Maximum consecutive connection failures before removing a bootstrap peer
const MAX_BOOTSTRAP_FAILURES: u32 = 3;
const COLD_BOOTSTRAP_FAILURES: u32 = 10;

#[derive(Parser, Debug, Clone)]
#[command(name = "shard-daemon", version, about = "Shard P2P Daemon")]
pub struct Cli {
    #[command(subcommand)]
    command: Option<DaemonCommand>,

    /// Port for the embedded HTTP control-plane API
    #[arg(long, default_value = "9091")]
    control_port: u16,

    /// Port for read-only telemetry WebSocket stream
    #[arg(long, default_value = "9093")]
    telemetry_ws_port: u16,

    /// TCP transport listen port
    #[arg(long, default_value = "4001")]
    tcp_port: u16,

    /// UDP port for WebRTC-direct (non-Windows only)
    #[arg(long, default_value = "9090")]
    webrtc_port: u16,

    /// UDP port for QUIC/WebTransport-ready transport
    #[arg(long, default_value = "9092")]
    quic_port: u16,

    /// Bootstrap peer multiaddr (can be repeated)
    #[arg(long)]
    bootstrap_node: Vec<String>,

    /// Path to newline-delimited bootstrap multiaddrs
    #[arg(long)]
    bootstrap_file: Option<String>,

    /// URL to fetch bootstrap peers from (returns JSON array of {peer_id, multiaddr})
    #[arg(long)]
    bootstrap_url: Option<String>,

    /// URL to register as a bootstrap peer (POST with peer info when stable)
    #[arg(long)]
    bootstrap_advertise_url: Option<String>,

    /// Minimum hours of uptime before advertising as a bootstrap peer
    #[arg(long, default_value = "1")]
    stability_threshold_hours: u64,

    /// Seconds between reconnect attempts to known peers
    #[arg(long, default_value = "20")]
    reconnect_seconds: u64,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Expose API publicly (allow external connections)
    #[arg(long, default_value = "false")]
    public_api: bool,

    /// Public hostname/IP for API exposure (auto-detected if not set)
    #[arg(long)]
    public_host: Option<String>,

    /// Run as circuit relay server (help other peers behind NAT)
    #[arg(long, default_value = "false")]
    relay_mode: bool,

    /// Contribute compute to the network (run as Shard node)
    #[arg(long, default_value = "true")]
    contribute: bool,

    /// Enable high-availability mode (leader election active)
    #[arg(long, default_value = "false")]
    ha_mode: bool,

    /// Enable private mode and enforce network_policy.yaml restrictions.
    #[arg(long, default_value = "false")]
    private_mode: bool,

    /// Enable NAT traversal (circuit relay + hole punching)
    #[arg(long, default_value = "true")]
    nat_traversal: bool,

    /// Hosted model identifier for layer routing announcements.
    #[arg(long, default_value = "meta-llama/Llama-3.2-1B")]
    model_id: String,

    /// First transformer layer hosted by this node.
    #[arg(long, default_value = "0")]
    layer_start: u32,

    /// Last transformer layer hosted by this node (inclusive).
    #[arg(long, default_value = "0")]
    layer_end: u32,

    /// Maximum peers to race for the next-layer activation.
    #[arg(long, default_value = "3")]
    race_pool_size: usize,

    /// Race timeout in milliseconds.
    #[arg(long, default_value = "3000")]
    race_timeout_ms: u64,

    /// Comma-separated list of STUN/TURN server URIs
    #[arg(long, value_delimiter = ',')]
    ice_servers: Vec<String>,

    /// URL to fetch ICE/TURN servers dynamically (e.g. from Twilio or custom service)
    #[arg(long)]
    ice_provider_url: Option<String>,
}

#[derive(Subcommand, Debug, Clone)]
enum DaemonCommand {
    Wallet {
        #[command(subcommand)]
        command: WalletCommand,
    },
    Model {
        #[command(subcommand)]
        command: ModelCommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum WalletCommand {
    Show,
    Export(WalletExportArgs),
    Import(WalletImportArgs),
    VerifyBackup(WalletVerifyArgs),
}

#[derive(Subcommand, Debug, Clone)]
enum ModelCommand {
    List,
    Pull { model_id: String },
    Verify { model_id: String },
    Remove { model_id: String },
}

#[derive(Args, Debug, Clone)]
struct WalletExportArgs {
    #[arg(long)]
    out: PathBuf,
    #[arg(long, default_value = "SHARD_WALLET_PASSWORD")]
    password_env: String,
    #[arg(long, default_value_t = 65_536)]
    kdf_memory_kib: u32,
    #[arg(long, default_value_t = 3)]
    kdf_iterations: u32,
    #[arg(long, default_value_t = 1)]
    kdf_parallelism: u32,
}

#[derive(Args, Debug, Clone)]
struct WalletImportArgs {
    #[arg(long = "in")]
    in_path: PathBuf,
    #[arg(long, default_value = "SHARD_WALLET_PASSWORD")]
    password_env: String,
    #[arg(long, default_value_t = false)]
    force: bool,
}

#[derive(Args, Debug, Clone)]
struct WalletVerifyArgs {
    #[arg(long = "in")]
    in_path: PathBuf,
    #[arg(long, default_value = "SHARD_WALLET_PASSWORD")]
    password_env: String,
}

// ─── Protocol Messages ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Heartbeat {
    kind: String,
    sent_at_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelManifest {
    schema_version: String,
    updated_at: String,
    models: Vec<ModelManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelManifestEntry {
    id: String,
    display_name: String,
    version: String,
    sha256: String,
    size_bytes: u64,
    download_url: String,
    min_vram_gb: u64,
    min_ram_gb: u64,
    roles: Vec<String>,
    quantization: String,
    architecture: String,
    release_notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DraftSubmission {
    task_id: String,
    scout_peer_id: String,
    seq_start: u32,
    draft_tokens: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct PrivateMeshRegisterRequest {
    api_key: String,
    node_pubkey_hex: String,
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PrivateMeshDeregisterRequest {
    api_key: String,
    node_pubkey_hex: String,
}

#[derive(Debug, Deserialize)]
struct PrivateMeshRouteRequest {
    api_key: String,
    #[serde(default)]
    connected_peers: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AdminApiKeyRequest {
    #[serde(default)]
    key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ScoutIngressUpdateRequest {
    enabled: bool,
}

fn generate_api_key() -> String {
    let suffix: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    format!("sk-{suffix}")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TensorDataFormat {
    Fp16,
    Fp32,
    Bf16,
    Quantized,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensorChunkRef {
    pub chunk_index: u32,
    pub total_chunks: u32,
    pub byte_size: u64,
    pub checksum_blake3: Option<String>,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensorBlobRef {
    pub uri: String,
    pub byte_size: u64,
    pub checksum_blake3: Option<String>,
    #[serde(default)]
    pub expires_at_ms: Option<u128>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardPassActivation {
    pub request_id: String,
    pub step_id: String,
    pub source_peer_id: String,
    pub target_peer_id: Option<String>,
    #[serde(default)]
    pub target_peer_pool: Option<Vec<String>>,
    pub tensor_name: String,
    pub shape: Vec<usize>,
    pub format: TensorDataFormat,
    #[serde(default)]
    pub chunk: Option<TensorChunkRef>,
    #[serde(default)]
    pub blob_ref: Option<TensorBlobRef>,
    #[serde(default)]
    pub created_at_ms: Option<u128>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackwardPassGradient {
    pub request_id: String,
    pub step_id: String,
    pub microbatch_id: String,
    pub source_peer_id: String,
    pub target_peer_id: Option<String>,
    pub layer_path: String,
    pub tensor_name: String,
    pub shape: Vec<usize>,
    pub format: TensorDataFormat,
    #[serde(default)]
    pub chunk: Option<TensorChunkRef>,
    #[serde(default)]
    pub blob_ref: Option<TensorBlobRef>,
    #[serde(default)]
    pub created_at_ms: Option<u128>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "packet_type", content = "payload", rename_all = "snake_case")]
pub enum TrainingGossipPacket {
    ForwardPass(ForwardPassActivation),
    BackwardPass(BackwardPassGradient),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedPeers {
    peers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BootstrapRegistryEntry {
    peer_id: String,
    multiaddr: String,
    stability_score: u32,
    uptime_hours: u64,
    version: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    capability_tier: Option<String>,
    #[serde(default)]
    gpu_available: Option<bool>,
    #[serde(default)]
    accepts_scout_work: Option<bool>,
    #[serde(default)]
    public_api: Option<bool>,
    #[serde(default)]
    public_api_addr: Option<String>,
    updated_at_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedBootstrapRegistry {
    entries: Vec<BootstrapRegistryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BootstrapAnnouncement {
    peer_id: String,
    multiaddr: String,
    stability_score: u32,
    uptime_hours: u64,
    version: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    capability_tier: Option<String>,
    #[serde(default)]
    gpu_available: Option<bool>,
    #[serde(default)]
    accepts_scout_work: Option<bool>,
    #[serde(default)]
    public_api: Option<bool>,
    #[serde(default)]
    public_api_addr: Option<String>,
    announced_at_ms: u128,
}

// ─── Shared State ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize)]
struct TopologyState {
    local_peer_id: String,
    listen_addrs: Vec<String>,
    webrtc_addr: Option<String>,
    quic_addr: Option<String>,
    ws_addr: Option<String>,
    public_api_addr: Option<String>,
    is_public: bool,
    relay_server_enabled: bool,
    relay_reservation_active: bool,
    nat_status: String,
    contribute_enabled: bool,
    capacity: u32,   // tokens per second
    load: u32,       // current active requests
    latency_ms: f32, // average response latency in ms
}

#[derive(Clone, Debug, Serialize)]
struct PeerInfo {
    peer_id: String,
    connected_at: u128,
    last_seen_at: u128,
    addrs: Vec<String>,
    verified: bool,
    handshake_failures: u32,
    /// First time this peer was seen (for stability tracking)
    first_seen_at: u128,
    /// Number of successful handshakes
    successful_handshakes: u32,
    /// Average latency in ms
    avg_latency_ms: f32,
    /// Number of consecutive connection failures (for bootstrap removal)
    pub connection_failures: u32,
}

#[derive(Debug, Clone)]
struct PeerReconnectStats {
    stability_score: u32,
    successful_handshakes: u32,
    connection_failures: u32,
    avg_latency_ms: f32,
    bootstrap_failures: u32,
    is_cold: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransportKind {
    Tcp,
    Websocket,
    Quic,
    Webrtc,
    Relay,
    Unknown,
}

#[derive(Clone)]
pub(crate) struct SharedState {
    topology: Arc<Mutex<TopologyState>>,
    peers: Arc<Mutex<HashMap<String, PeerInfo>>>,
    known_peers: Arc<Mutex<Vec<String>>>,
    known_peers_path: PathBuf,
    results: Arc<Mutex<VecDeque<WorkResponse>>>,
    scout_work: Arc<Mutex<VecDeque<WorkRequest>>>,
    work_tx: mpsc::Sender<WorkRequest>,
    pipeline_tx: mpsc::Sender<PipelineDispatch>,
    browser_result_tx: mpsc::Sender<ForwardPassActivation>,
    daemon_start: u128,
    capacity: Arc<AtomicU32>,
    current_load: Arc<AtomicU32>,
    avg_latency_ms: Arc<AtomicU32>,
    fast_verifier_bypass_until_ms: Arc<AtomicU64>,
    gossipsub_latency_hist: Arc<LatencyHistogram>,
    credit_nonce: Arc<AtomicU64>,
    scout_penalties: Arc<Mutex<ScoutPenaltyBook>>,
    backward_passes: Arc<Mutex<VecDeque<BackwardPassGradient>>>,
    layer_routes: Arc<Mutex<LayerRoutingTable>>,
    race_router: Arc<Mutex<RaceRouter>>,
    ledger: Arc<Mutex<LedgerState>>,
    ledger_store: Arc<LedgerStore>,
    browser_sessions: Arc<Mutex<HashMap<String, BrowserLayerSession>>>,
    scout_client_runtime: Arc<Mutex<HashMap<String, ScoutClientRuntimeStatus>>>,
    scout_work_last_poll: Arc<Mutex<HashMap<String, u128>>>,
    scout_draft_last_submit: Arc<Mutex<HashMap<String, u128>>>,
    scout_work_leases: Arc<Mutex<HashMap<String, ScoutWorkLease>>>,
    scout_blackout: Arc<Mutex<ScoutBlackoutState>>,
    webgpu_stats: Arc<Mutex<WebGPUStats>>,
    browser_work: Arc<Mutex<VecDeque<BrowserLayerWorkItem>>>,
    node_wallet: String,
    model_id: String,
    layer_start: u32,
    layer_end: u32,
    race_pool_size: usize,
    race_timeout_ms: u64,
    engine: Arc<Mutex<Option<shard_verifier::inference::ShardEngine>>>,
    replay_nonces: Arc<Mutex<HashMap<String, u64>>>,
    system_metrics: Arc<SystemMetrics>,
    node_metric_reports: Arc<Mutex<HashMap<String, NodeMetricSnapshot>>>,
    metrics_persistence: Arc<MetricsPersistence>,
    heartbeat_timeout_ms: u128,
    idempotent_results: Arc<Mutex<HashMap<String, WorkResponse>>>,
    node_reputation: Arc<Mutex<HashMap<String, NodeReputation>>>,
    node_reputation_path: PathBuf,
    node_role: String,
    participation_enabled: Arc<AtomicBool>,
    resource_policy: ResourcePolicy,
    event_log: Arc<Mutex<VecDeque<String>>>,
    node_public_key: String,
    heartbeat_interval_seconds: Arc<AtomicU64>,
    scout_timeout_ms: Arc<AtomicU64>,
    max_scouts: Arc<AtomicUsize>,
    acceptance_threshold_bps: Arc<AtomicU64>,
    public_host: Option<String>,
    tcp_port: u16,
    webrtc_port: u16,
    quic_port: u16,
    /// Active generation requests for fallback tracking (B3).
    active_requests: Arc<Mutex<HashMap<String, ActiveRequestState>>>,
    /// Fallback configuration (B2/B3).
    fallback_config: Arc<FallbackConfig>,
    /// Stateful envelope verifier with replay protection (A1).
    envelope_verifier: Arc<Mutex<EnvelopeVerifier>>,
    /// PoW challenge manager for Sybil resistance (A5).
    pow_manager: Arc<Mutex<PowChallengeManager>>,
    /// Private mesh registry for enterprise prompt privacy (C4).
    private_mesh: Arc<Mutex<PrivateMeshRegistry>>,
    /// Alert manager for anomaly detection (D3).
    alert_manager: Arc<Mutex<AlertManager>>,
    /// API keys for clients (managed by admin endpoint).
    api_keys: Arc<Mutex<HashSet<String>>>,
    /// Rate limiter for API requests
    rate_limiter: Arc<shard_gateway::rate_limiter::RateLimiter>,
    /// Optional admin token for managing API keys.
    admin_key: Option<String>,
    /// Whether API keys are required for chat completions.
    require_api_key: bool,
    /// Node signing key for PoC receipts
    signing_key: ed25519_dalek::SigningKey,
    /// WebRTC ICE servers (STUN/TURN)
    /// WebRTC ICE servers (STUN/TURN)
    ice_servers: Arc<Mutex<Vec<String>>>,
    /// Channel for receiving scout draft submissions
    scout_draft_tx: mpsc::Sender<ScoutDraft>,
    #[allow(dead_code)]
    scout_draft_rx: Arc<Mutex<Option<mpsc::Receiver<ScoutDraft>>>>,
    /// Per-work mailbox for deterministic draft handoff to verifier waiters.
    scout_draft_mailbox: Arc<Mutex<HashMap<String, VecDeque<ScoutDraft>>>>,
    /// Per-work notifiers used by waiters to wake when matching drafts arrive.
    scout_draft_notifiers: Arc<Mutex<HashMap<String, Arc<Notify>>>>,
    /// Pending speculative requests keyed by work/request id with issue timestamp.
    speculative_pending: Arc<Mutex<HashMap<String, u128>>>,
    /// Recent terminal outcomes for speculative requests, used to classify late drafts.
    speculative_terminal: Arc<Mutex<HashMap<String, SpeculativeTerminalState>>>,
    /// Bounded in-memory trace of speculative lifecycle events.
    speculative_trace: Arc<Mutex<VecDeque<SpeculativeTraceEvent>>>,
    /// Buffered draft segments for out-of-order speculative submissions.
    draft_buffers: Arc<Mutex<HashMap<String, DraftBuffer>>>,
    /// Channel for announcing bans to the network
    ban_tx: mpsc::Sender<(String, String)>,
    /// Timeout tracker for speculative decoding
    scout_timeout_tracker: Arc<Mutex<ScoutTimeoutTracker>>,
    /// Channel for daemon-side scout workers to publish generated drafts
    /// back to the main event loop for gossipsub broadcast.
    draft_publish_tx: mpsc::Sender<WorkResponse>,
    /// Bootstrap peer failure tracking (peer_id -> consecutive failures)
    /// Used to remove unreachable bootstraps after MAX_BOOTSTRAP_FAILURES
    bootstrap_failures: Arc<Mutex<HashMap<String, u32>>>,
    bootstrap_ring: Option<Arc<bootstrap_ring::BootstrapRing>>,
    network_policy: Option<Arc<NetworkPolicy>>,
    private_mode: bool,
    bootstrap_registry: Arc<Mutex<HashMap<String, BootstrapRegistryEntry>>>,
    bootstrap_registry_path: PathBuf,
    scheduler_decisions: Arc<Mutex<VecDeque<SchedulerDecisionLog>>>,
    mesh_probe_backoff: Arc<Mutex<HashMap<String, (u32, u128)>>>,
    canary_rollout: Arc<Mutex<CanaryRolloutController>>,
    scout_ingress_enabled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    in_flight_count: Arc<AtomicUsize>,
    /// EWMA of scout draft arrival latency in ms (for adaptive wait budget).
    avg_draft_arrival_ms: Arc<AtomicU32>,
    /// EWMA of accepted draft tokens ×100 for fixed-point precision.
    avg_accepted_tokens_x100: Arc<AtomicU32>,
    consensus: Option<Arc<LeaderElectionHandle>>,
}

fn env_live_scout_timeout_ms() -> u64 {
    std::env::var("SHARD_SCOUT_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|v| v.clamp(100, 60_000))
        .unwrap_or(2_000)
}

fn env_live_max_scouts() -> usize {
    std::env::var("SHARD_MAX_SCOUTS")
        .ok()
        .or_else(|| std::env::var("SHARD_SCOUT_ACTIVE_CAP").ok())
        .and_then(|v| v.trim().parse::<usize>().ok())
        .map(|v| v.clamp(1, 256))
        .unwrap_or(8)
}

fn env_live_acceptance_threshold_bps() -> u64 {
    let threshold = std::env::var("SHARD_ACCEPTANCE_THRESHOLD")
        .ok()
        .or_else(|| std::env::var("SHARD_SPECULATIVE_BYPASS_THRESHOLD").ok())
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map(|v| v.clamp(0.0, 1.0))
        .unwrap_or(0.20);
    (threshold * 10_000.0).round() as u64
}

pub(crate) fn acceptance_threshold_from_bps(raw: u64) -> f64 {
    (raw.min(10_000) as f64) / 10_000.0
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct BanAnnouncement {
    pub peer_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
struct ResourcePolicy {
    max_cpu_usage: f32,
    max_gpu_usage: f32,
    idle_only_mode: bool,
    load_threshold_cutoff: f32,
}

// ─── Speculative Decoding Types ────────────────────────────────────────────

/// Configuration for speculative decoding (Scout draft verification).
#[derive(Clone, Debug)]
struct SpeculativeConfig {
    /// Timeout in ms to wait for scout draft (default: 1500ms).
    scout_timeout_ms: u64,
    /// Cooldown in ms after 3 consecutive timeouts (default: 60000ms).
    scout_cooldown_ms: u64,
    /// Number of consecutive timeouts before cooldown.
    max_consecutive_timeouts: u32,
    /// Number of draft tokens to request from scout.
    draft_token_count: usize,
}

const SPECULATIVE_TRACE_MAX_EVENTS: usize = 4096;
const SPECULATIVE_TERMINAL_TTL_MS: u128 = 2 * 60 * 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SpeculativeTerminalState {
    pub outcome: String,
    pub observed_at_ms: u128,
    #[serde(default)]
    pub scout_id: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SpeculativeTraceEvent {
    pub request_id: String,
    pub stage: String,
    pub at_ms: u128,
    #[serde(default)]
    pub scout_id: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub pending_age_ms: Option<u64>,
}

pub(crate) async fn record_speculative_trace(
    state: &SharedState,
    request_id: impl Into<String>,
    stage: impl Into<String>,
    scout_id: Option<String>,
    detail: Option<String>,
    pending_age_ms: Option<u64>,
) {
    let event = SpeculativeTraceEvent {
        request_id: request_id.into(),
        stage: stage.into(),
        at_ms: now_ms(),
        scout_id,
        detail,
        pending_age_ms,
    };
    let mut trace = state.speculative_trace.lock().await;
    trace.push_back(event);
    while trace.len() > SPECULATIVE_TRACE_MAX_EVENTS {
        trace.pop_front();
    }
}

pub(crate) async fn prune_speculative_terminal_state(state: &SharedState, now: u128) {
    let mut terminal = state.speculative_terminal.lock().await;
    terminal
        .retain(|_, entry| now.saturating_sub(entry.observed_at_ms) <= SPECULATIVE_TERMINAL_TTL_MS);
}

pub(crate) async fn set_speculative_terminal_state(
    state: &SharedState,
    request_id: &str,
    outcome: &str,
    scout_id: Option<String>,
    detail: Option<String>,
) {
    let now = now_ms();
    {
        let mut terminal = state.speculative_terminal.lock().await;
        terminal.insert(
            request_id.to_string(),
            SpeculativeTerminalState {
                outcome: outcome.to_string(),
                observed_at_ms: now,
                scout_id: scout_id.clone(),
                detail: detail.clone(),
            },
        );
        terminal.retain(|_, entry| {
            now.saturating_sub(entry.observed_at_ms) <= SPECULATIVE_TERMINAL_TTL_MS
        });
    }
    record_speculative_trace(
        state,
        request_id.to_string(),
        format!("terminal:{outcome}"),
        scout_id,
        detail,
        None,
    )
    .await;
}

pub(crate) async fn speculative_terminal_state(
    state: &SharedState,
    request_id: &str,
) -> Option<SpeculativeTerminalState> {
    prune_speculative_terminal_state(state, now_ms()).await;
    let terminal = state.speculative_terminal.lock().await;
    terminal.get(request_id).cloned()
}

impl Default for SpeculativeConfig {
    fn default() -> Self {
        Self {
            scout_timeout_ms: std::env::var("SHARD_SCOUT_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                // Browser scouts poll work over HTTP and may need PoW + queueing +
                // submission retries. Keep default high enough to avoid dropping
                // valid drafts as late mismatches.
                .unwrap_or(1_500),
            scout_cooldown_ms: 60000,
            max_consecutive_timeouts: 3,
            draft_token_count: std::env::var("SHARD_SCOUT_DRAFT_TOKEN_COUNT")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .map(|v| v.clamp(1, 16))
                .unwrap_or(4),
        }
    }
}

/// Tracks consecutive scout timeouts for cooldown management.
#[derive(Clone, Debug)]
struct ScoutTimeoutTracker {
    consecutive_timeouts: u32,
    cooldown_until_ms: u128,
}

impl ScoutTimeoutTracker {
    fn new() -> Self {
        Self {
            consecutive_timeouts: 0,
            cooldown_until_ms: 0,
        }
    }

    fn record_timeout(&mut self, config: &SpeculativeConfig) {
        self.consecutive_timeouts += 1;
        if self.consecutive_timeouts >= config.max_consecutive_timeouts {
            self.cooldown_until_ms = now_ms() + (config.scout_cooldown_ms as u128);
            tracing::warn!(
                "scout cooldown triggered: {} consecutive timeouts",
                self.consecutive_timeouts
            );
        }
    }

    fn record_success(&mut self) {
        self.consecutive_timeouts = 0;
    }

    fn is_in_cooldown(&self) -> bool {
        now_ms() < self.cooldown_until_ms
    }
}

#[derive(Clone, Debug)]
struct ScoutWorkLease {
    lease_id: String,
    scout_id: String,
    expires_at_ms: u128,
}

#[derive(Clone, Debug, Default)]
struct ScoutBlackoutState {
    overload_since_ms: Option<u128>,
    blackout_until_ms: u128,
    reopen_started_ms: Option<u128>,
}

/// A draft submission from a Scout browser node.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ScoutDraft {
    work_id: String,
    scout_id: String,
    draft_tokens: Vec<i32>,
    draft_text: String,
    latency_ms: u64,
    timestamp_ms: u128,
}

#[derive(Debug, Clone)]
struct DraftBufferConfig {
    max_segments: usize,
    max_gap_tokens: u32,
    min_emit_tokens: usize,
    ttl_ms: u128,
}

#[derive(Debug, Clone)]
struct DraftBuffer {
    next_seq: u32,
    assembled: Vec<i32>,
    segments: BTreeMap<u32, Vec<i32>>,
    last_update_ms: u128,
}

fn draft_buffer_config() -> DraftBufferConfig {
    DraftBufferConfig {
        max_segments: std::env::var("SHARD_DRAFT_BUFFER_MAX_SEGMENTS")
            .ok()
            .and_then(|v| v.parse().ok())
            .map(|v: usize| v.clamp(1, 32))
            .unwrap_or(8),
        max_gap_tokens: std::env::var("SHARD_DRAFT_BUFFER_MAX_GAP_TOKENS")
            .ok()
            .and_then(|v| v.parse().ok())
            .map(|v: u32| v.clamp(16, 4096))
            .unwrap_or(256),
        min_emit_tokens: std::env::var("SHARD_DRAFT_BUFFER_MIN_TOKENS")
            .ok()
            .and_then(|v| v.parse().ok())
            .map(|v: usize| v.clamp(1, 4096))
            .unwrap_or(8),
        ttl_ms: std::env::var("SHARD_DRAFT_BUFFER_TTL_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .map(|v: u128| v.clamp(5_000, 120_000))
            .unwrap_or(30_000),
    }
}

/// Result of verifying draft tokens against the verifier model.
#[derive(Clone, Debug)]
struct DraftVerificationResult {
    accepted_tokens: Vec<i32>,
    accepted_text: String,
    first_rejection_idx: Option<usize>,
    #[allow(dead_code)]
    resample_token: Option<i32>,
}

async fn capture_alert<F>(state: &SharedState, f: F)
where
    F: FnOnce(&mut AlertManager) -> Option<&Alert>,
{
    let mut manager = state.alert_manager.lock().await;
    if let Some(alert) = f(&mut manager) {
        let alert = alert.clone();
        drop(manager);
        tracing::warn!(
            kind = ?alert.kind,
            severity = ?alert.severity,
            value = ?alert.value,
            threshold = ?alert.threshold,
            message = %alert.message,
            "alert triggered"
        );
    }
}

async fn record_request_alert(state: &SharedState) {
    capture_alert(state, |manager| manager.on_request()).await;
}

async fn record_latency_alert(state: &SharedState, latency_ms: f64) {
    let latency_ms = latency_ms.max(0.0).round() as u64;
    capture_alert(state, |manager| manager.on_latency(latency_ms)).await;
}

async fn push_scout_draft(state: &SharedState, draft: ScoutDraft) {
    let work_id = draft.work_id.clone();
    {
        let mut mailbox = state.scout_draft_mailbox.lock().await;
        let queue = mailbox.entry(work_id.clone()).or_insert_with(VecDeque::new);
        queue.push_back(draft);
        while queue.len() > 8 {
            queue.pop_front();
        }
    }
    {
        let notifiers = state.scout_draft_notifiers.lock().await;
        if let Some(notify) = notifiers.get(&work_id) {
            notify.notify_waiters();
        }
    }
}

async fn process_draft_submission(
    state: &SharedState,
    submission: DraftSubmission,
) -> Option<ScoutDraft> {
    if submission.draft_tokens.is_empty() {
        return None;
    }
    let config = draft_buffer_config();
    let now = now_ms();
    let key = format!("{}:{}", submission.task_id, submission.scout_peer_id);
    let mut buffers = state.draft_buffers.lock().await;
    buffers.retain(|_, buffer| now.saturating_sub(buffer.last_update_ms) <= config.ttl_ms);
    let buffer = buffers.entry(key.clone()).or_insert_with(|| DraftBuffer {
        next_seq: 0,
        assembled: Vec::new(),
        segments: BTreeMap::new(),
        last_update_ms: now,
    });
    buffer.last_update_ms = now;

    if submission.seq_start > buffer.next_seq.saturating_add(config.max_gap_tokens) {
        return None;
    }

    let tokens: Vec<i32> = submission
        .draft_tokens
        .into_iter()
        .map(|t| t as i32)
        .collect();
    if buffer.segments.len() >= config.max_segments {
        if let Some((&oldest, _)) = buffer.segments.iter().next() {
            buffer.segments.remove(&oldest);
        }
    }
    buffer.segments.insert(submission.seq_start, tokens);

    while let Some(segment) = buffer.segments.remove(&buffer.next_seq) {
        buffer.next_seq = buffer.next_seq.saturating_add(segment.len() as u32);
        buffer.assembled.extend(segment);
    }

    if buffer.assembled.len() >= config.min_emit_tokens {
        let assembled = std::mem::take(&mut buffer.assembled);
        buffers.remove(&key);
        return Some(ScoutDraft {
            work_id: submission.task_id,
            scout_id: submission.scout_peer_id,
            draft_tokens: assembled,
            draft_text: String::new(),
            latency_ms: 0,
            timestamp_ms: now,
        });
    }
    None
}

async fn record_signature_alert(state: &SharedState, success: bool) {
    capture_alert(state, |manager| manager.on_signature_verification(success)).await;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScoutPenaltyUpdate {
    peer_id: String,
    accepted: bool,
    probability_bound: f64,
    latency_ms: Option<u64>,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DraftResultSubmission {
    work_id: String,
    scout_id: String,
    #[serde(default)]
    lease_id: Option<String>,
    draft_text: String,
    #[serde(default)]
    prompt_context: Option<String>,
    #[serde(default)]
    draft_tokens: Vec<i32>,
    #[serde(default)]
    timestamp: Option<f64>,
    #[serde(default)]
    scout_mode: Option<String>,
    #[serde(default)]
    spot_check: Option<DraftSpotCheckProof>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DraftSpotCheckProof {
    input_a: Vec<f32>,
    weights_b: Vec<f32>,
    claimed_c: Vec<f32>,
    m: usize,
    k: usize,
    n: usize,
    #[serde(default)]
    seed: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignedRequest<T> {
    envelope: SignedEnvelope<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NodeRegistration {
    node_pubkey: String,
    role: String,
    capacity: Option<u64>,
    #[serde(default)]
    timestamp_ms: Option<u128>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NodeHeartbeat {
    node_pubkey: String,
    role: String,
    queue_depth: u64,
    node_latency_ms: u64,
    uptime_seconds: u64,
    #[serde(default)]
    capability_tier: Option<String>,
    #[serde(default)]
    gpu_available: Option<bool>,
    #[serde(default)]
    accepts_scout_work: Option<bool>,
    #[serde(default)]
    public_api: Option<bool>,
    #[serde(default)]
    public_api_addr: Option<String>,
    #[serde(default)]
    timestamp_ms: Option<u128>,
}

#[derive(Debug, Deserialize)]
struct PopResultQuery {
    request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NextLayerQuery {
    model_id: Option<String>,
    current_layer: u32,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ForwardResultQuery {
    request_id: Option<String>,
    step_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ParticipationToggle {
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct LedgerExportQuery {
    from_height: Option<u64>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct PipelineForwardRequest {
    model_id: Option<String>,
    current_layer: u32,
    packet: ForwardPassActivation,
}

#[derive(Debug, Clone)]
struct PipelineDispatch {
    model_id: String,
    current_layer: u32,
    packet: ForwardPassActivation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SchedulerDecisionInput {
    node_id: String,
    load: f64,
    latency_ms: f64,
    reliability_score: f64,
    hardware_capability_score: f64,
    identity_reputation_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SchedulerDecisionLog {
    timestamp_ms: u128,
    model_id: String,
    current_layer: u32,
    next_layer: u32,
    candidate_peers: Vec<String>,
    selected_peers: Vec<String>,
    inputs: Vec<SchedulerDecisionInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BrowserLayerProfile {
    supports_webgpu: bool,
    webgpu_vendor: Option<String>,
    max_buffer_size_mb: Option<u32>,
    max_storage_buffers_per_stage: Option<u32>,
    user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BrowserLayerRegisterRequest {
    model_id: Option<String>,
    layer_start: u32,
    layer_end: u32,
    profile: BrowserLayerProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BrowserLayerRegisterResponse {
    ok: bool,
    session_id: String,
    model_id: String,
    layer_start: u32,
    layer_end: u32,
    expires_at_ms: u128,
    obfuscation_key_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BrowserLayerSession {
    session_id: String,
    model_id: String,
    layer_start: u32,
    layer_end: u32,
    profile: BrowserLayerProfile,
    obfuscation_key: Vec<u8>,
    last_seen_ms: u128,
    expires_at_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ScoutClientRuntimeStatus {
    pub scout_id: String,
    pub runtime_mode: Option<String>,
    pub last_event: String,
    pub last_event_detail: Option<String>,
    pub last_event_ms: u128,
    pub last_submit_success_ms: Option<u128>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WebGPUProbeResult {
    pub eligible: bool,
    #[serde(default)]
    pub reason: Option<String>,
    pub tier: String,
    pub estimated_vram_mb: u64,
    pub supports_f16: bool,
    pub browser: String,
    pub os: String,
    pub adapter_vendor: String,
    pub adapter_device: String,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct WebGPUStats {
    pub total_probes: u64,
    pub eligible: u64,
    pub high_performance: u64,
    pub low_power: u64,
    pub ineligible_reasons: HashMap<String, u64>,
    pub browser_counts: HashMap<String, u64>,
    pub os_counts: HashMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BrowserLayerWorkItem {
    work_id: String,
    session_id: String,
    request_id: String,
    step_id: String,
    source_peer_id: String,
    tensor_name: String,
    dtype: u8,
    shape: Vec<u32>,
    nonce_hex: String,
    obfuscated_tensor_hex: String,
    created_at_ms: u128,
}

#[derive(Debug, Deserialize)]
struct BrowserLayerWorkQuery {
    session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BrowserLayerResultSubmit {
    session_id: String,
    work_id: String,
    request_id: String,
    step_id: String,
    source_peer_id: String,
    tensor_name: String,
    dtype: u8,
    shape: Vec<u32>,
    nonce_hex: String,
    obfuscated_tensor_hex: String,
}

#[derive(Debug, Deserialize)]
struct WsGenerateRequest {
    request_id: Option<String>,
    prompt: Option<String>,
    prompt_context: Option<String>,
    max_new_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
struct ScoutPenaltyStatus {
    peer_id: String,
    score: i32,
    failures: u32,
    accepted: u32,
    blackholed: bool,
    success_rate: f32,
    p95_latency_ms: u64,
    last_reason: Option<String>,
}

#[derive(Debug, Default)]
struct ScoutReputationEntry {
    recent: VecDeque<bool>,
    failure_count: u32,
    consecutive_failures: u32,
    accepted_count: u32,
    latency_ms: u64,
    banned_until_ms: Option<u128>,
    last_reason: Option<String>,
}

#[derive(Debug, Default)]
struct ScoutPenaltyBook {
    entries: HashMap<String, ScoutReputationEntry>,
}

impl ScoutPenaltyBook {
    const WINDOW_SIZE: usize = 16;
    const MIN_SAMPLES_FOR_BAN: usize = 8;
    const MIN_CONSECUTIVE_FAILURES_FOR_BAN: u32 = 4;
    const SUCCESS_RATE_THRESHOLD: f32 = 0.45;
    const HIGH_LATENCY_MULTIPLIER: f64 = 2.5;
    const BAN_COOLDOWN_MS: u128 = 60_000;

    fn success_rate(entry: &ScoutReputationEntry) -> f32 {
        if entry.recent.is_empty() {
            return 1.0;
        }
        let success = entry.recent.iter().filter(|ok| **ok).count() as f32;
        success / (entry.recent.len() as f32)
    }

    fn apply_update(&mut self, update: ScoutPenaltyUpdate, global_p95: u64) -> ScoutPenaltyStatus {
        let now = now_ms();
        let entry = self.entries.entry(update.peer_id.clone()).or_default();
        if let Some(lat) = update.latency_ms {
            entry.latency_ms = lat;
        }

        let blackholed = entry
            .banned_until_ms
            .map(|until| until > now)
            .unwrap_or(false);

        if entry.recent.len() >= Self::WINDOW_SIZE {
            entry.recent.pop_front();
        }
        entry.recent.push_back(update.accepted);

        if update.accepted {
            entry.accepted_count = entry.accepted_count.saturating_add(1);
            entry.consecutive_failures = 0;
        } else {
            entry.failure_count = entry.failure_count.saturating_add(1);
            entry.consecutive_failures = entry.consecutive_failures.saturating_add(1);
            if let Some(reason) = update.reason.as_ref() {
                entry.last_reason = Some(reason.clone());
            }
        }

        let success_rate = Self::success_rate(entry);
        if entry.recent.len() >= Self::MIN_SAMPLES_FOR_BAN
            && entry.consecutive_failures >= Self::MIN_CONSECUTIVE_FAILURES_FOR_BAN
            && success_rate < Self::SUCCESS_RATE_THRESHOLD
        {
            entry.banned_until_ms = Some(now + Self::BAN_COOLDOWN_MS);
            entry.last_reason = Some(format!(
                "Low success rate: {:.2} over {} samples",
                success_rate,
                entry.recent.len()
            ));
        }

        if !blackholed
            && global_p95 > 0
            && entry.latency_ms > (global_p95 as f64 * Self::HIGH_LATENCY_MULTIPLIER) as u64
            && entry.consecutive_failures >= Self::MIN_CONSECUTIVE_FAILURES_FOR_BAN
        {
            entry.banned_until_ms = Some(now + Self::BAN_COOLDOWN_MS);
            entry.last_reason = Some(format!(
                "High latency: {}ms (P95: {}ms)",
                entry.latency_ms, global_p95
            ));
        }

        // Re-calculate blackholed after potentially banning
        let blackholed = entry
            .banned_until_ms
            .map(|until| until > now)
            .unwrap_or(false);

        if !blackholed {
            entry.banned_until_ms = None;
        }

        ScoutPenaltyStatus {
            peer_id: update.peer_id.clone(),
            score: (success_rate * 100.0).round() as i32,
            failures: entry.failure_count,
            accepted: entry.accepted_count,
            blackholed,
            success_rate,
            p95_latency_ms: entry.latency_ms,
            last_reason: entry.last_reason.clone(),
        }
    }

    fn is_blackholed(&mut self, peer_id: &str) -> bool {
        let now = now_ms();
        if let Some(entry) = self.entries.get_mut(peer_id) {
            if let Some(until) = entry.banned_until_ms {
                if until > now {
                    return true;
                }
                entry.banned_until_ms = None;
            }
        }
        false
    }

    pub(crate) fn quality_snapshot(&self, peer_id: &str) -> Option<(i32, usize)> {
        self.entries.get(peer_id).map(|entry| {
            (
                (Self::success_rate(entry) * 100.0).round() as i32,
                entry.recent.len(),
            )
        })
    }

    fn all_statuses(&self) -> Vec<ScoutPenaltyStatus> {
        self.entries
            .iter()
            .map(|(peer_id, entry)| ScoutPenaltyStatus {
                peer_id: peer_id.clone(),
                score: (Self::success_rate(entry) * 100.0).round() as i32,
                failures: entry.failure_count,
                accepted: entry.accepted_count,
                blackholed: entry
                    .banned_until_ms
                    .map(|until| until > now_ms())
                    .unwrap_or(false),
                success_rate: Self::success_rate(entry),
                p95_latency_ms: entry.latency_ms,
                last_reason: entry.last_reason.clone(),
            })
            .collect()
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn should_reject_peer_connection(penalties: &mut ScoutPenaltyBook, peer_id: &str) -> bool {
    penalties.is_blackholed(peer_id)
}

#[derive(Debug, Serialize)]
struct LatencyPercentiles {
    p50_ms: u64,
    p90_ms: u64,
    p95_ms: u64,
    p99_ms: u64,
    samples: u64,
}

#[derive(Debug)]
struct LatencyHistogram {
    /// Upper-bound bucket edges in milliseconds. Values above the last edge
    /// are stored in an overflow bucket.
    bucket_bounds_ms: [u64; 12],
    bucket_counts: [AtomicU64; 13],
}

impl LatencyHistogram {
    fn new() -> Self {
        Self {
            bucket_bounds_ms: [5, 10, 25, 50, 100, 150, 200, 300, 500, 1000, 2000, 5000],
            bucket_counts: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }

    fn observe(&self, latency_ms: u64) {
        for (idx, bound) in self.bucket_bounds_ms.iter().enumerate() {
            if latency_ms <= *bound {
                self.bucket_counts[idx].fetch_add(1, Ordering::Relaxed);
                return;
            }
        }
        self.bucket_counts[self.bucket_counts.len() - 1].fetch_add(1, Ordering::Relaxed);
    }

    fn reset(&self) {
        for bucket in &self.bucket_counts {
            bucket.store(0, Ordering::Relaxed);
        }
    }

    fn percentiles(&self) -> LatencyPercentiles {
        let counts: Vec<u64> = self
            .bucket_counts
            .iter()
            .map(|v| v.load(Ordering::Relaxed))
            .collect();
        let total: u64 = counts.iter().sum();
        if total == 0 {
            return LatencyPercentiles {
                p50_ms: 0,
                p90_ms: 0,
                p95_ms: 0,
                p99_ms: 0,
                samples: 0,
            };
        }

        let p50_target = ((total as f64) * 0.50).ceil() as u64;
        let p90_target = ((total as f64) * 0.90).ceil() as u64;
        let p95_target = ((total as f64) * 0.95).ceil() as u64;
        let p99_target = ((total as f64) * 0.99).ceil() as u64;

        let mut running = 0u64;
        let mut p50 = 0u64;
        let mut p90 = 0u64;
        let mut p95 = 0u64;
        let mut p99 = 0u64;

        for (idx, count) in counts.iter().enumerate() {
            running += *count;
            let bucket_upper = self.bucket_bounds_ms.get(idx).copied().unwrap_or(10_000);

            if p50 == 0 && running >= p50_target {
                p50 = bucket_upper;
            }
            if p90 == 0 && running >= p90_target {
                p90 = bucket_upper;
            }
            if p95 == 0 && running >= p95_target {
                p95 = bucket_upper;
            }
            if p99 == 0 && running >= p99_target {
                p99 = bucket_upper;
                break;
            }
        }

        LatencyPercentiles {
            p50_ms: p50,
            p90_ms: p90,
            p95_ms: p95,
            p99_ms: p99,
            samples: total,
        }
    }
}

// ─── libp2p Behaviour ───────────────────────────────────────────────────────

#[derive(NetworkBehaviour)]
struct ShardBehaviour {
    gossipsub: gossipsub::Behaviour,
    kad: KadBehaviour<MemoryStore>,
    handshake: request_response::cbor::Behaviour<Heartbeat, Heartbeat>,
    verify: request_response::cbor::Behaviour<DraftSubmission, String>,
    control_work: request_response::cbor::Behaviour<WorkRequest, String>,
    ledger_sync: request_response::cbor::Behaviour<LedgerSyncRequest, LedgerSyncResponse>,
    relay_server: libp2p::swarm::behaviour::toggle::Toggle<relay::Behaviour>,
    relay_client: libp2p::swarm::behaviour::toggle::Toggle<relay::client::Behaviour>,
    dcutr: libp2p::swarm::behaviour::toggle::Toggle<dcutr::Behaviour>,
    autonat: autonat::v1::Behaviour,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
    mdns: libp2p::mdns::tokio::Behaviour,
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

fn data_dir() -> std::path::PathBuf {
    if let Ok(override_dir) = std::env::var("SHARD_DATA_DIR") {
        let trimmed = override_dir.trim();
        if !trimmed.is_empty() {
            return std::path::PathBuf::from(trimmed);
        }
    }
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("shard")
}

async fn ensure_data_dir() -> Result<std::path::PathBuf> {
    let primary = data_dir();
    match tokio::fs::create_dir_all(&primary).await {
        Ok(_) => Ok(primary),
        Err(primary_error) => {
            let fallback = std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join(".shard-data");
            tokio::fs::create_dir_all(&fallback)
                .await
                .map_err(|fallback_error| {
                    anyhow::anyhow!(
                        "failed to initialize data dir at {} ({}) and fallback {} ({})",
                        primary.display(),
                        primary_error,
                        fallback.display(),
                        fallback_error
                    )
                })?;
            tracing::warn!(
                primary = %primary.display(),
                fallback = %fallback.display(),
                error = %primary_error,
                "using fallback data directory after primary data dir initialization failure"
            );
            Ok(fallback)
        }
    }
}

fn extract_peer_id_from_multiaddr(addr: &Multiaddr) -> Option<PeerId> {
    use libp2p::multiaddr::Protocol;
    for protocol in addr.iter() {
        if let Protocol::P2p(peer_id) = protocol {
            return Some(peer_id);
        }
    }
    None
}

fn peer_id_from_addr_str(addr: &str) -> Option<String> {
    addr.parse::<Multiaddr>().ok().and_then(|multiaddr| {
        extract_peer_id_from_multiaddr(&multiaddr).map(|peer| peer.to_string())
    })
}

fn ip_from_addr_str(addr: &str) -> Option<IpAddr> {
    use libp2p::multiaddr::Protocol;
    let parsed = addr.parse::<Multiaddr>().ok()?;
    for protocol in parsed.iter() {
        match protocol {
            Protocol::Ip4(ipv4) => return Some(IpAddr::V4(ipv4)),
            Protocol::Ip6(ipv6) => return Some(IpAddr::V6(ipv6)),
            _ => {}
        }
    }
    None
}

fn transport_kind_from_text(raw: &str) -> TransportKind {
    let text = raw.to_ascii_lowercase();
    if text.contains("/p2p-circuit") || text.contains("circuit") || text.contains("relay") {
        return TransportKind::Relay;
    }
    if text.contains("/webrtc-direct") || text.contains("webrtc") {
        return TransportKind::Webrtc;
    }
    if text.contains("/quic-v1") || text.contains("quic") {
        return TransportKind::Quic;
    }
    if text.contains("/ws") || text.contains("websocket") {
        return TransportKind::Websocket;
    }
    if text.contains("/tcp/") || text.contains(" tcp ") {
        return TransportKind::Tcp;
    }
    TransportKind::Unknown
}

fn record_transport_success(metrics: &SystemMetrics, kind: TransportKind) {
    match kind {
        TransportKind::Tcp => metrics.inc_transport_tcp_success(),
        TransportKind::Websocket => metrics.inc_transport_websocket_success(),
        TransportKind::Quic => metrics.inc_transport_quic_success(),
        TransportKind::Webrtc => metrics.inc_transport_webrtc_success(),
        TransportKind::Relay => metrics.inc_transport_relay_success(),
        TransportKind::Unknown => {}
    }
}

fn record_transport_failure(metrics: &SystemMetrics, kind: TransportKind) {
    match kind {
        TransportKind::Tcp => metrics.inc_transport_tcp_failure(),
        TransportKind::Websocket => metrics.inc_transport_websocket_failure(),
        TransportKind::Quic => metrics.inc_transport_quic_failure(),
        TransportKind::Webrtc => metrics.inc_transport_webrtc_failure(),
        TransportKind::Relay => metrics.inc_transport_relay_failure(),
        TransportKind::Unknown => {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HardcodedBootstrapMode {
    Always,
    Fallback,
    Disabled,
}

fn parse_hardcoded_bootstrap_mode(raw: Option<String>) -> HardcodedBootstrapMode {
    match raw
        .unwrap_or_else(|| "fallback".to_string())
        .trim()
        .to_lowercase()
        .as_str()
    {
        "always" => HardcodedBootstrapMode::Always,
        "disabled" | "disable" | "off" | "false" | "none" => HardcodedBootstrapMode::Disabled,
        _ => HardcodedBootstrapMode::Fallback,
    }
}

fn should_include_hardcoded_bootstrap(
    mode: HardcodedBootstrapMode,
    has_user_bootstrap: bool,
) -> bool {
    match mode {
        HardcodedBootstrapMode::Always => true,
        HardcodedBootstrapMode::Disabled => false,
        HardcodedBootstrapMode::Fallback => !has_user_bootstrap,
    }
}

fn bootstrap_registry_ttl_ms() -> u128 {
    std::env::var("SHARD_BOOTSTRAP_REGISTRY_TTL_MS")
        .ok()
        .and_then(|value| value.parse::<u128>().ok())
        .filter(|value| *value >= 60_000)
        .unwrap_or(24 * 60 * 60 * 1000)
}

fn bootstrap_registry_min_score() -> u32 {
    std::env::var("SHARD_BOOTSTRAP_REGISTRY_MIN_SCORE")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .map(|value| value.min(100))
        .unwrap_or(30)
}

fn bootstrap_url_refresh_interval_ms() -> u128 {
    std::env::var("SHARD_BOOTSTRAP_URL_REFRESH_MS")
        .ok()
        .and_then(|value| value.parse::<u128>().ok())
        .filter(|value| *value >= 60_000)
        .unwrap_or(15 * 60 * 1000)
}

fn bootstrap_gossip_interval_ms() -> u128 {
    std::env::var("SHARD_BOOTSTRAP_GOSSIP_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u128>().ok())
        .filter(|value| *value >= 60_000)
        .unwrap_or(10 * 60 * 1000)
}

fn prune_bootstrap_registry(
    registry: &mut HashMap<String, BootstrapRegistryEntry>,
    now_ms: u128,
    ttl_ms: u128,
) -> Vec<String> {
    let mut removed_peer_ids = Vec::new();
    registry.retain(|peer_id, entry| {
        let is_fresh = now_ms.saturating_sub(entry.updated_at_ms) <= ttl_ms;
        if !is_fresh {
            removed_peer_ids.push(peer_id.clone());
        }
        is_fresh
    });
    removed_peer_ids
}

fn bootstrap_registry_seed_addrs(
    registry: &HashMap<String, BootstrapRegistryEntry>,
    now_ms: u128,
    ttl_ms: u128,
    min_score: u32,
) -> Vec<String> {
    let mut entries = registry
        .values()
        .filter(|entry| now_ms.saturating_sub(entry.updated_at_ms) <= ttl_ms)
        .filter(|entry| entry.stability_score >= min_score)
        .cloned()
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| {
        b.stability_score
            .cmp(&a.stability_score)
            .then(b.updated_at_ms.cmp(&a.updated_at_ms))
            .then(b.uptime_hours.cmp(&a.uptime_hours))
    });
    unique_addrs(entries.into_iter().map(|entry| entry.multiaddr).collect())
}

fn preferred_bootstrap_multiaddr(addrs: &[String]) -> Option<String> {
    let mut fallbacks = Vec::new();
    for addr in addrs {
        let Ok(parsed) = addr.parse::<Multiaddr>() else {
            continue;
        };
        let is_local = parsed.to_string().contains("127.0.0.1")
            || parsed.to_string().contains("::1")
            || parsed.to_string().contains("/ip4/0.0.0.0/")
            || parsed.to_string().contains("/ip6/::/");
        if is_local {
            continue;
        }
        if !is_non_public_bootstrap_addr(&parsed) {
            return Some(addr.clone());
        }
        fallbacks.push(addr.clone());
    }
    fallbacks.into_iter().next()
}

fn canonical_bootstrap_multiaddr(addr: &str, peer_id: &str) -> String {
    let trimmed = addr.trim();
    if trimmed.is_empty() || peer_id.trim().is_empty() || trimmed.contains("/p2p/") {
        return trimmed.to_string();
    }
    format!("{trimmed}/p2p/{peer_id}")
}

pub(crate) async fn upsert_bootstrap_entry(
    state: &SharedState,
    mut entry: BootstrapRegistryEntry,
) -> bool {
    entry.multiaddr = canonical_bootstrap_multiaddr(&entry.multiaddr, &entry.peer_id);
    let mut registry_changed = false;
    {
        let mut registry = state.bootstrap_registry.lock().await;
        let replace = registry
            .get(entry.peer_id.as_str())
            .map(|existing| {
                existing.updated_at_ms < entry.updated_at_ms
                    || existing.multiaddr != entry.multiaddr
                    || existing.stability_score != entry.stability_score
                    || existing.uptime_hours != entry.uptime_hours
                    || existing.version != entry.version
                    || existing.role != entry.role
                    || existing.capability_tier != entry.capability_tier
                    || existing.gpu_available != entry.gpu_available
                    || existing.accepts_scout_work != entry.accepts_scout_work
                    || existing.public_api != entry.public_api
                    || existing.public_api_addr != entry.public_api_addr
            })
            .unwrap_or(true);
        if replace {
            registry.insert(entry.peer_id.clone(), entry.clone());
            save_bootstrap_registry(state.bootstrap_registry_path.as_path(), &registry).await;
            registry_changed = true;
        }
    }

    let mut known_changed = false;
    {
        let mut known = state.known_peers.lock().await;
        let before = known.len();
        known.push(entry.multiaddr.clone());
        *known = unique_addrs(known.clone());
        if known.len() != before {
            save_persisted_peers(state.known_peers_path.as_path(), &known).await;
            known_changed = true;
        }
    }

    registry_changed || known_changed
}

fn remove_known_addrs_for_peers(known: &mut Vec<String>, peer_ids: &HashSet<String>) -> usize {
    if peer_ids.is_empty() {
        return 0;
    }
    let before = known.len();
    known.retain(|addr| {
        peer_id_from_addr_str(addr)
            .map(|peer_id| !peer_ids.contains(&peer_id))
            .unwrap_or(true)
    });
    before.saturating_sub(known.len())
}

fn should_attempt_reconnect(
    addr: &Multiaddr,
    local_peer_id: &PeerId,
    connected: &HashSet<String>,
) -> bool {
    let is_self = addr.to_string().contains(&local_peer_id.to_string());
    if is_self {
        return false;
    }
    if let Some(peer_id) = extract_peer_id_from_multiaddr(addr) {
        return !connected.contains(&peer_id.to_string());
    }
    false
}

fn is_non_public_bootstrap_addr(addr: &Multiaddr) -> bool {
    for proto in addr.iter() {
        match proto {
            libp2p::multiaddr::Protocol::Ip4(ip) => {
                let [a, b, _, _] = ip.octets();
                if a == 10
                    || (a == 172 && (16..=31).contains(&b))
                    || (a == 192 && b == 168)
                    || a == 127
                    || (a == 169 && b == 254)
                {
                    return true;
                }
            }
            libp2p::multiaddr::Protocol::Ip6(ip) => {
                if ip.is_loopback()
                    || ip.is_unspecified()
                    || ip.is_unique_local()
                    || ip.is_unicast_link_local()
                {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn filter_bootstrap_addrs(addrs: Vec<String>, allow_private: bool) -> Vec<String> {
    unique_addrs(addrs)
        .into_iter()
        .filter(|addr| {
            let Ok(multiaddr) = addr.parse::<Multiaddr>() else {
                return true;
            };
            let has_peer = multiaddr
                .iter()
                .any(|p| matches!(p, libp2p::multiaddr::Protocol::P2p(_)));
            if !has_peer {
                tracing::warn!(%addr, "dropping bootstrap address missing /p2p/<peer>");
                return false;
            }
            if !allow_private && is_non_public_bootstrap_addr(&multiaddr) {
                tracing::warn!(%addr, "dropping non-public bootstrap address; set SHARD_ALLOW_PRIVATE_BOOTSTRAP=true to allow");
                return false;
            }
            true
        })
        .collect()
}

fn filter_relay_addrs(addrs: Vec<String>, allow_private: bool) -> Vec<String> {
    unique_addrs(addrs)
        .into_iter()
        .filter(|addr| {
            let Ok(multiaddr) = addr.parse::<Multiaddr>() else {
                return false;
            };
            let has_peer = multiaddr
                .iter()
                .any(|p| matches!(p, libp2p::multiaddr::Protocol::P2p(_)));
            if !has_peer {
                tracing::warn!(%addr, "dropping relay address missing /p2p/<peer>");
                return false;
            }
            if !allow_private && is_non_public_bootstrap_addr(&multiaddr) {
                tracing::warn!(%addr, "dropping non-public relay address; set SHARD_ALLOW_PRIVATE_RELAY=true to allow");
                return false;
            }
            true
        })
        .collect()
}

fn build_autonat_config() -> autonat::v1::Config {
    let mut config = autonat::v1::Config::default();
    let confidence_max = std::env::var("SHARD_AUTONAT_CONFIDENCE_MAX")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map(|value| value.clamp(3, 10))
        .unwrap_or(5);
    let retry_secs = std::env::var("SHARD_AUTONAT_RETRY_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(|value| value.clamp(30, 600))
        .unwrap_or(180);
    let refresh_secs = std::env::var("SHARD_AUTONAT_REFRESH_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(|value| value.clamp(300, 3600))
        .unwrap_or(1800);

    config.confidence_max = confidence_max;
    config.retry_interval = Duration::from_secs(retry_secs);
    config.refresh_interval = Duration::from_secs(refresh_secs);
    config
}

fn reconnect_transport_priority(addr: &Multiaddr) -> u8 {
    let text = addr.to_string();
    // Relay circuit paths remain the highest-priority recovery path.
    if text.contains("/p2p-circuit") {
        return 0;
    }
    if text.contains("/quic-v1") {
        return 1;
    }
    if text.contains("/tcp/") {
        return 2;
    }
    if text.contains("/webrtc-direct") {
        return 3;
    }
    if text.contains("/wss/") || text.contains("/ws/") {
        return 4;
    }
    5
}

fn is_reconnect_candidate_addr(addr: &Multiaddr, allow_private: bool) -> bool {
    let text = addr.to_string();
    if text.contains("/wss/") || text.contains("/ws/") {
        return false;
    }
    let has_peer = addr
        .iter()
        .any(|p| matches!(p, libp2p::multiaddr::Protocol::P2p(_)));
    if !has_peer {
        return false;
    }
    if !allow_private && is_non_public_bootstrap_addr(addr) {
        return false;
    }
    true
}

fn reconnect_addr_sort_key(addr_str: &str) -> (u8, String) {
    if let Ok(addr) = addr_str.parse::<Multiaddr>() {
        let mut priority = reconnect_transport_priority(&addr);
        if is_non_public_bootstrap_addr(&addr) {
            priority = priority.saturating_add(10);
        }
        return (priority, addr_str.to_string());
    }
    (u8::MAX, addr_str.to_string())
}

fn node_capability_tier(role: &str, max_gpu_usage: f32, relay_mode: bool, public_api: bool) -> String {
    if role.eq_ignore_ascii_case("scout") {
        return "scout_only".to_string();
    }
    if relay_mode && !public_api {
        return "relay_only".to_string();
    }
    if max_gpu_usage >= 0.75 {
        "gpu_fast".to_string()
    } else if max_gpu_usage >= 0.4 {
        "cpu_standard".to_string()
    } else {
        "cpu_slow".to_string()
    }
}

fn node_gpu_available_for_tier(capability_tier: &str) -> bool {
    capability_tier.starts_with("gpu_")
}

fn node_accepts_scout_work(role: &str, participation_enabled: bool, relay_mode: bool) -> bool {
    participation_enabled && !role.eq_ignore_ascii_case("scout") && !relay_mode
}

fn reconnect_backoff_ms_for_failures(failures: u32) -> u128 {
    if failures >= COLD_BOOTSTRAP_FAILURES {
        return 60 * 60 * 1000;
    }
    ((20_000u128) << failures.min(4)).min(300_000)
}

pub(crate) fn mesh_probe_backoff_ms_for_failures(failures: u32) -> u128 {
    ((15_000u128) << failures.min(6)).min(15 * 60 * 1000)
}

fn peer_reconnect_stats_for_addr(
    addr_str: &str,
    registry: &HashMap<String, BootstrapRegistryEntry>,
    peers: &HashMap<String, PeerInfo>,
    bootstrap_failures: &HashMap<String, u32>,
) -> PeerReconnectStats {
    let peer_id = peer_id_from_addr_str(addr_str);
    let peer_snapshot = peer_id
        .as_ref()
        .and_then(|id| peers.get(id));
    let registry_entry = peer_id
        .as_ref()
        .and_then(|id| registry.get(id));
    let bootstrap_failure_count = peer_id
        .as_ref()
        .and_then(|id| bootstrap_failures.get(id))
        .copied()
        .unwrap_or(0);
    PeerReconnectStats {
        stability_score: registry_entry.map(|entry| entry.stability_score).unwrap_or(0),
        successful_handshakes: peer_snapshot
            .map(|peer| peer.successful_handshakes)
            .unwrap_or(0),
        connection_failures: peer_snapshot
            .map(|peer| peer.connection_failures)
            .unwrap_or(0),
        avg_latency_ms: peer_snapshot.map(|peer| peer.avg_latency_ms).unwrap_or(0.0),
        bootstrap_failures: bootstrap_failure_count,
        is_cold: bootstrap_failure_count >= COLD_BOOTSTRAP_FAILURES,
    }
}

fn max_reconnect_dials_per_tick() -> usize {
    static MAX_DIALS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *MAX_DIALS.get_or_init(|| {
        std::env::var("SHARD_MAX_RECONNECT_DIALS_PER_TICK")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .map(|value| value.clamp(1, 64))
            .unwrap_or(8)
    })
}

fn record_bootstrap_failure(
    _known: &mut Vec<String>,
    failures: &mut HashMap<String, u32>,
    peer_id: &PeerId,
) -> bool {
    let peer_id_str = peer_id.to_string();
    let is_bootstrap = _known.iter().any(|addr| {
        if let Ok(multiaddr) = addr.parse::<libp2p::Multiaddr>() {
            extract_peer_id_from_multiaddr(&multiaddr) == Some(*peer_id)
        } else {
            false
        }
    });
    if !is_bootstrap {
        return false;
    }

    let count = failures.entry(peer_id_str.clone()).or_insert(0);
    *count += 1;
    if *count >= MAX_BOOTSTRAP_FAILURES {
        // Keep bootstrap peers in the known set to preserve recovery after
        // restarts/transient WAN faults. Reconnect backoff + cold demotion
        // handle retry pacing instead of hard deletion.
        *count = (*count).min(COLD_BOOTSTRAP_FAILURES);
        return false;
    }
    false
}

fn resolve_metrics_persistence(data: &Path) -> MetricsPersistence {
    match std::env::var("SHARD_METRICS_BACKEND")
        .unwrap_or_else(|_| "sqlite".to_string())
        .to_lowercase()
        .as_str()
    {
        "none" => MetricsPersistence::None,
        "postgres" => std::env::var("SHARD_METRICS_POSTGRES_URL")
            .map(|dsn| MetricsPersistence::Postgres { dsn })
            .unwrap_or(MetricsPersistence::None),
        _ => {
            let path = std::env::var("SHARD_METRICS_SQLITE_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| data.join("metrics.db"));
            MetricsPersistence::Sqlite { path }
        }
    }
}

fn wallet_password_from_env(env_name: &str) -> Result<String> {
    let value = std::env::var(env_name).map_err(|_| {
        anyhow::anyhow!(
            "missing wallet password env var {env_name}; set it before running wallet command"
        )
    })?;
    if value.is_empty() {
        return Err(anyhow::anyhow!("wallet password cannot be empty"));
    }
    Ok(value)
}

fn handle_wallet_command(command: WalletCommand, identity_path: &Path) -> Result<()> {
    match command {
        WalletCommand::Show => {
            let identity = NodeIdentity::load_or_create(identity_path)?;
            println!("wallet_address={}", identity.wallet_address());
        }
        WalletCommand::Export(args) => {
            let password = wallet_password_from_env(&args.password_env)?;
            let wallet = export_wallet(
                identity_path,
                &args.out,
                &password,
                args.kdf_memory_kib,
                args.kdf_iterations,
                args.kdf_parallelism,
            )?;
            println!("exported_wallet={wallet}");
            println!("backup_file={}", args.out.display());
        }
        WalletCommand::Import(args) => {
            let password = wallet_password_from_env(&args.password_env)?;
            let wallet = import_wallet(&args.in_path, identity_path, &password, args.force)?;
            println!("imported_wallet={wallet}");
            println!("identity_file={}", identity_path.display());
        }
        WalletCommand::VerifyBackup(args) => {
            let password = wallet_password_from_env(&args.password_env)?;
            let wallet = verify_backup(&args.in_path, &password)?;
            println!("verified_wallet={wallet}");
            println!("backup_file={}", args.in_path.display());
        }
    }
    Ok(())
}

fn model_manifest_url() -> String {
    std::env::var("SHARD_MODEL_MANIFEST_URL").unwrap_or_else(|_| {
        "https://raw.githubusercontent.com/TrentPierce/Shard/main/deploy/models/manifest.json"
            .to_string()
    })
}

fn model_store_dir(base_data_dir: &Path) -> PathBuf {
    base_data_dir.join("models")
}

async fn fetch_model_manifest() -> Result<ModelManifest> {
    let url = model_manifest_url();
    let response = reqwest::get(url.clone())
        .await
        .map_err(|e| anyhow::anyhow!("failed to fetch manifest from {url}: {e}"))?;
    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("manifest fetch failed with status {}", status);
    }
    let manifest = response
        .json::<ModelManifest>()
        .await
        .map_err(|e| anyhow::anyhow!("failed to decode model manifest: {e}"))?;
    Ok(manifest)
}

fn sha256_file(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        sha2::Digest::update(&mut hasher, &buf[..read]);
    }
    Ok(hex::encode(sha2::Digest::finalize(hasher)))
}

fn model_local_path(base_data_dir: &Path, entry: &ModelManifestEntry) -> PathBuf {
    let filename = entry
        .download_url
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("model.bin");
    model_store_dir(base_data_dir)
        .join(entry.id.as_str())
        .join(entry.version.as_str())
        .join(filename)
}

async fn handle_model_command(command: ModelCommand, data_dir: &Path) -> Result<()> {
    match command {
        ModelCommand::List => {
            let manifest = fetch_model_manifest().await?;
            println!(
                "{:<24} {:<10} {:<12} {:<8}",
                "MODEL ID", "VERSION", "SIZE(B)", "LOCAL"
            );
            for model in manifest.models {
                let path = model_local_path(data_dir, &model);
                let local = if path.exists() { "yes" } else { "no" };
                println!(
                    "{:<24} {:<10} {:<12} {:<8}",
                    model.id, model.version, model.size_bytes, local
                );
            }
        }
        ModelCommand::Pull { model_id } => {
            let manifest = fetch_model_manifest().await?;
            let Some(model) = manifest.models.into_iter().find(|m| m.id == model_id) else {
                anyhow::bail!("model '{model_id}' not found in manifest");
            };
            let final_path = model_local_path(data_dir, &model);
            if final_path.exists() {
                let existing_hash = sha256_file(final_path.as_path())?;
                if existing_hash.eq_ignore_ascii_case(model.sha256.as_str()) {
                    println!(
                        "model already downloaded and verified: {}",
                        final_path.display()
                    );
                    return Ok(());
                }
            }
            if let Some(parent) = final_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let tmp_path = final_path.with_extension("download");

            let response = reqwest::get(model.download_url.as_str()).await?;
            if !response.status().is_success() {
                anyhow::bail!(
                    "download failed for {} with status {}",
                    model.id,
                    response.status()
                );
            }
            let bytes = response.bytes().await?;
            std::fs::write(tmp_path.as_path(), bytes.as_ref())?;
            let got = sha256_file(tmp_path.as_path())?;
            if !got.eq_ignore_ascii_case(model.sha256.as_str()) {
                let _ = std::fs::remove_file(tmp_path.as_path());
                anyhow::bail!(
                    "sha256 mismatch for model {} (expected {}, got {})",
                    model.id,
                    model.sha256,
                    got
                );
            }
            std::fs::rename(tmp_path.as_path(), final_path.as_path())?;
            println!("downloaded model {} to {}", model.id, final_path.display());
        }
        ModelCommand::Verify { model_id } => {
            let manifest = fetch_model_manifest().await?;
            let Some(model) = manifest.models.into_iter().find(|m| m.id == model_id) else {
                anyhow::bail!("model '{model_id}' not found in manifest");
            };
            let path = model_local_path(data_dir, &model);
            if !path.exists() {
                anyhow::bail!("model not found locally: {}", path.display());
            }
            let got = sha256_file(path.as_path())?;
            if got.eq_ignore_ascii_case(model.sha256.as_str()) {
                println!("verify ok: {} {}", model.id, model.version);
            } else {
                anyhow::bail!("verify failed: expected {}, got {}", model.sha256, got);
            }
        }
        ModelCommand::Remove { model_id } => {
            let target = model_store_dir(data_dir).join(model_id.as_str());
            if target.exists() {
                std::fs::remove_dir_all(target.as_path())?;
                println!("removed model {}", model_id);
            } else {
                println!("model not present: {}", model_id);
            }
        }
    }
    Ok(())
}

fn parse_semver_like(version: &str) -> (u64, u64, u64) {
    let mut parts = version.split('.');
    let major = parts
        .next()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let minor = parts
        .next()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let patch = parts
        .next()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    (major, minor, patch)
}

fn is_newer_version(current: &str, candidate: &str) -> bool {
    parse_semver_like(candidate) > parse_semver_like(current)
}

fn unique_addrs(addrs: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for a in addrs {
        if seen.insert(a.clone()) {
            out.push(a);
        }
    }
    out
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn should_enable_ipv6_p2p_listeners() -> bool {
    env_flag("SHARD_P2P_LISTEN_IPV6")
        || std::env::var_os("FLY_APP_NAME").is_some()
        || std::env::var_os("FLY_PRIVATE_IP").is_some()
}

fn normalize_public_host(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let no_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let host_port = no_scheme.split('/').next().unwrap_or(no_scheme);
    let host = if host_port.starts_with('[') {
        host_port
            .trim_start_matches('[')
            .split(']')
            .next()
            .unwrap_or("")
    } else {
        host_port.split(':').next().unwrap_or(host_port)
    };
    let host = host.trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

fn fly_private_ipv6() -> Option<String> {
    std::env::var("FLY_PRIVATE_IP")
        .ok()
        .map(|value| value.trim().trim_matches(['[', ']']).to_string())
        .filter(|value| value.parse::<std::net::Ipv6Addr>().is_ok())
}

fn p2p_listen_multiaddrs(cli: &Cli) -> anyhow::Result<Vec<Multiaddr>> {
    let mut addrs = vec![
        format!("/ip4/0.0.0.0/tcp/{}", cli.tcp_port).parse()?,
        format!("/ip4/0.0.0.0/tcp/{}/ws", cli.tcp_port + 100).parse()?,
        format!("/ip4/0.0.0.0/udp/{}/webrtc-direct", cli.webrtc_port).parse()?,
        format!("/ip4/0.0.0.0/udp/{}/quic-v1", cli.quic_port).parse()?,
    ];
    if let Some(ipv6) = fly_private_ipv6() {
        addrs.push(format!("/ip6/{ipv6}/tcp/{}", cli.tcp_port).parse()?);
        addrs.push(format!("/ip6/{ipv6}/tcp/{}/ws", cli.tcp_port + 100).parse()?);
        addrs.push(format!("/ip6/{ipv6}/udp/{}/webrtc-direct", cli.webrtc_port).parse()?);
        addrs.push(format!("/ip6/{ipv6}/udp/{}/quic-v1", cli.quic_port).parse()?);
    }
    Ok(addrs)
}

fn host_multiaddr_prefix(host: &str) -> (String, String) {
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(_)) => ("ip4".to_string(), host.to_string()),
        Ok(IpAddr::V6(_)) => ("ip6".to_string(), host.to_string()),
        Err(_) if host.ends_with(".internal") || host.ends_with(".internal.") => {
            ("dns6".to_string(), host.to_string())
        }
        Err(_) => ("dns4".to_string(), host.to_string()),
    }
}

fn rewrite_multiaddr_host(addr: &str, host_proto: &str, host_value: &str) -> Option<String> {
    let mut parts: Vec<String> = addr
        .split('/')
        .filter(|p| !p.is_empty())
        .map(|s| s.to_string())
        .collect();
    if parts.len() < 2 {
        return None;
    }
    match parts[0].as_str() {
        "ip4" | "ip6" | "dns4" | "dns6" => {}
        _ => return None,
    }
    parts[0] = host_proto.to_string();
    parts[1] = host_value.to_string();
    Some(format!("/{}", parts.join("/")))
}

fn outward_topology_addrs(
    topo: &TopologyState,
    state: &SharedState,
) -> (Option<String>, Option<String>, Option<String>, Vec<String>) {
    let mut ws_addr = topo.ws_addr.clone();
    let mut webrtc_addr = topo.webrtc_addr.clone();
    let mut quic_addr = topo.quic_addr.clone();
    let mut listen_addrs = topo.listen_addrs.clone();

    if topo.is_public {
        if let Some(host_raw) = state
            .public_host
            .as_deref()
            .or(topo.public_api_addr.as_deref())
            .and_then(normalize_public_host)
        {
            let (proto, host_value) = host_multiaddr_prefix(&host_raw);
            if let Some(addr) = ws_addr
                .as_deref()
                .and_then(|v| rewrite_multiaddr_host(v, &proto, &host_value))
            {
                ws_addr = Some(addr);
            } else {
                ws_addr = Some(format!(
                    "/{}/{}/tcp/{}/ws/p2p/{}",
                    proto,
                    host_value,
                    state.tcp_port.saturating_add(100),
                    topo.local_peer_id
                ));
            }
            if let Some(addr) = webrtc_addr
                .as_deref()
                .and_then(|v| rewrite_multiaddr_host(v, &proto, &host_value))
            {
                webrtc_addr = Some(addr);
            } else {
                webrtc_addr = Some(format!(
                    "/{}/{}/udp/{}/webrtc-direct/p2p/{}",
                    proto, host_value, state.webrtc_port, topo.local_peer_id
                ));
            }
            if let Some(addr) = quic_addr
                .as_deref()
                .and_then(|v| rewrite_multiaddr_host(v, &proto, &host_value))
            {
                quic_addr = Some(addr);
            } else {
                quic_addr = Some(format!(
                    "/{}/{}/udp/{}/quic-v1/p2p/{}",
                    proto, host_value, state.quic_port, topo.local_peer_id
                ));
            }

            let local_peer = topo.local_peer_id.clone();
            listen_addrs.retain(|addr| {
                !(addr.starts_with("/ip4/127.")
                    || addr.starts_with("/ip6/::1")
                    || addr.starts_with("/ip4/172.")
                    || addr.starts_with("/ip4/10.")
                    || addr.starts_with("/ip4/192.168."))
            });
            if let Some(ws) = &ws_addr {
                listen_addrs.push(ws.split("/p2p/").next().unwrap_or(ws.as_str()).to_string());
            } else {
                listen_addrs.push(format!(
                    "/{}/{}/tcp/{}/ws",
                    proto,
                    host_value,
                    state.tcp_port.saturating_add(100)
                ));
            }
            listen_addrs.push(format!(
                "/{}/{}/tcp/{}/p2p/{}",
                proto, host_value, state.tcp_port, local_peer
            ));
            if let Some(quic) = &quic_addr {
                listen_addrs.push(
                    quic.split("/p2p/")
                        .next()
                        .unwrap_or(quic.as_str())
                        .to_string(),
                );
            }
            listen_addrs = unique_addrs(listen_addrs);
        }
    }

    (webrtc_addr, quic_addr, ws_addr, listen_addrs)
}

async fn read_bootstrap_file(path: &str) -> Vec<String> {
    let Ok(contents) = tokio::fs::read_to_string(path).await else {
        return Vec::new();
    };

    contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect()
}

async fn load_persisted_peers(path: &Path) -> Vec<String> {
    let Ok(raw) = tokio::fs::read(path).await else {
        return Vec::new();
    };
    let Ok(parsed) = serde_json::from_slice::<PersistedPeers>(&raw) else {
        return Vec::new();
    };
    unique_addrs(parsed.peers)
}

async fn save_persisted_peers(path: &Path, peers: &[String]) {
    let payload = PersistedPeers {
        peers: unique_addrs(peers.to_vec()),
    };
    if let Ok(bytes) = serde_json::to_vec_pretty(&payload) {
        let _ = tokio::fs::write(path, bytes).await;
    }
}

async fn load_bootstrap_registry(path: &Path) -> HashMap<String, BootstrapRegistryEntry> {
    let Ok(raw) = tokio::fs::read(path).await else {
        return HashMap::new();
    };
    let Ok(parsed) = serde_json::from_slice::<PersistedBootstrapRegistry>(&raw) else {
        return HashMap::new();
    };
    parsed
        .entries
        .into_iter()
        .map(|mut entry| {
            entry.multiaddr = canonical_bootstrap_multiaddr(&entry.multiaddr, &entry.peer_id);
            (entry.peer_id.clone(), entry)
        })
        .collect()
}

async fn save_bootstrap_registry(path: &Path, registry: &HashMap<String, BootstrapRegistryEntry>) {
    let payload = PersistedBootstrapRegistry {
        entries: registry.values().cloned().collect(),
    };
    if let Ok(bytes) = serde_json::to_vec_pretty(&payload) {
        let _ = tokio::fs::write(path, bytes).await;
    }
}

async fn append_event_log(state: &SharedState, event: impl Into<String>) {
    let mut log = state.event_log.lock().await;
    log.push_back(format!("{} {}", now_ms(), event.into()));
    while log.len() > 200 {
        log.pop_front();
    }
}

fn should_accept_work(state: &SharedState) -> Result<(), String> {
    if !state.participation_enabled.load(Ordering::Relaxed) {
        return Err("node participation disabled".to_string());
    }
    let verifier_depth = verifier_request_depth(state);
    let verifier_cap = verifier_queue_cap(state);
    if verifier_depth >= verifier_cap {
        return Err(format!(
            "verifier queue saturated ({verifier_depth}/{verifier_cap})"
        ));
    }
    let capacity = state.capacity.load(Ordering::Relaxed).max(1);
    let load = state
        .current_load
        .load(Ordering::Relaxed)
        .max(verifier_depth as u32);
    let load_ratio = load as f32 / capacity as f32;

    if state.resource_policy.idle_only_mode && load > 0 {
        return Err("idle_only_mode enabled and node is busy".to_string());
    }
    if load_ratio > state.resource_policy.load_threshold_cutoff {
        return Err("load threshold cutoff reached".to_string());
    }
    Ok(())
}

fn verifier_queue_cap_base() -> usize {
    std::env::var("SHARD_VERIFIER_QUEUE_CAP")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .map(|v| v.clamp(1, 64))
        .unwrap_or(2)
}

pub(crate) fn verifier_request_depth(state: &SharedState) -> usize {
    let current_load = state.current_load.load(Ordering::Relaxed) as usize;
    let in_flight = state.in_flight_count.load(Ordering::Relaxed);
    current_load.max(in_flight)
}

pub(crate) fn verifier_queue_cap(state: &SharedState) -> usize {
    let _ = state;
    verifier_queue_cap_base()
}

pub(crate) struct VerifierLoadGuard {
    counter: Arc<AtomicU32>,
}

impl VerifierLoadGuard {
    pub(crate) fn try_acquire(state: &SharedState) -> Option<Self> {
        let counter = Arc::clone(&state.current_load);
        let cap = verifier_queue_cap(state) as u32;
        loop {
            let current = counter.load(Ordering::Relaxed);
            if current >= cap {
                return None;
            }
            if counter
                .compare_exchange(
                    current,
                    current.saturating_add(1),
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                return Some(Self { counter });
            }
        }
    }
}

impl Drop for VerifierLoadGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Relaxed);
    }
}

#[derive(Clone)]
struct ApiAccessPolicy {
    allowed_origins: Arc<HashSet<String>>,
    public_api: bool,
}

fn build_cors_origins(control_port: u16, public_host: Option<&str>) -> Vec<HeaderValue> {
    if let Ok(raw_origins) = std::env::var("SHARD_CORS_ORIGINS") {
        let origins: Vec<HeaderValue> = raw_origins
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .filter(|value| !value.eq_ignore_ascii_case("any") && *value != "*")
            .filter_map(|value| HeaderValue::from_str(value).ok())
            .collect();
        if !origins.is_empty() {
            return origins;
        }
    }

    // Secure-by-default CORS baseline; operators can override with SHARD_CORS_ORIGINS.
    let mut defaults = vec![
        format!("http://localhost:3000"),
        format!("http://127.0.0.1:3000"),
        format!("http://localhost:{control_port}"),
        format!("http://127.0.0.1:{control_port}"),
    ];
    if let Some(host) = public_host {
        defaults.push(format!("http://{host}:{control_port}"));
        defaults.push(format!("https://{host}:{control_port}"));
    }
    defaults
        .iter()
        .filter_map(|value| HeaderValue::from_str(value).ok())
        .collect()
}

fn host_is_local(host: &str) -> bool {
    let raw = host.trim().trim_start_matches('[').trim_end_matches(']');
    let host_only = raw.split(':').next().unwrap_or(raw);
    matches!(host_only, "localhost" | "127.0.0.1" | "::1")
}

async fn enforce_api_origin(
    AxumState(policy): AxumState<ApiAccessPolicy>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if !policy.public_api {
        if let Some(host) = req.headers().get("host").and_then(|v| v.to_str().ok()) {
            if !host_is_local(host) {
                return (StatusCode::FORBIDDEN, "host not allowed").into_response();
            }
        }
    }

    if let Some(origin) = req.headers().get("origin").and_then(|v| v.to_str().ok()) {
        if !policy.allowed_origins.contains(origin) {
            return (StatusCode::FORBIDDEN, "origin not allowed").into_response();
        }
    }

    next.run(req).await
}

async fn enforce_shutdown_and_track(
    AxumState(state): AxumState<SharedState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    if state.shutdown.load(Ordering::Relaxed) && path != "/health" && path != "/v1/system/health" {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "shutdown_in_progress",
                "message": "Node is draining in-flight requests. Retry shortly."
            })),
        )
            .into_response();
    }

    let _in_flight_guard =
        InFlightRequestGuard::new(track_in_flight_path(path.as_str(), &state.in_flight_count));
    let response = next.run(req).await;
    response
}

fn track_in_flight_path(path: &str, counter: &Arc<AtomicUsize>) -> Option<Arc<AtomicUsize>> {
    should_track_in_flight_path(path).then(|| Arc::clone(counter))
}

fn should_track_in_flight_path(path: &str) -> bool {
    matches!(
        path,
        "/v1/chat/completions"
            | "/ws/generate"
            | "/pipeline/forward"
            | "/broadcast-work"
            | "/signed/broadcast-work"
            | "/submit-draft"
            | "/v1/scout/draft"
            | "/signed/submit-draft"
            | "/browser-layer/submit"
    )
}

struct InFlightRequestGuard {
    counter: Option<Arc<AtomicUsize>>,
}

impl InFlightRequestGuard {
    fn new(counter: Option<Arc<AtomicUsize>>) -> Self {
        if let Some(counter_ref) = counter.as_ref() {
            counter_ref.fetch_add(1, Ordering::Relaxed);
        }
        Self { counter }
    }
}

impl Drop for InFlightRequestGuard {
    fn drop(&mut self) {
        if let Some(counter) = self.counter.as_ref() {
            counter.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

fn create_router(
    state: SharedState,
    cors_origins: Vec<HeaderValue>,
    policy: ApiAccessPolicy,
) -> Router {
    let mut cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    if !cors_origins.is_empty() {
        cors = cors.allow_origin(cors_origins);
    }

    Router::new()
        .layer(from_fn_with_state(
            state.clone(),
            enforce_shutdown_and_track,
        ))
        .layer(from_fn_with_state(policy, enforce_api_origin))
        .route("/health", get(health_handler))
        .route("/v1/system/health", get(health_handler))
        .route("/connectivity", get(connectivity_handler))
        .route("/v1/system/connectivity", get(connectivity_handler))
        .route("/topology", get(topology_handler))
        .route("/v1/system/topology", get(topology_handler))
        .route("/wallet/address", get(wallet_address_handler))
        .route("/node/status", get(node_status_handler))
        .route("/node/consensus-role", get(node_consensus_role_handler))
        .route("/node/ui", get(node_ui_handler))
        .route(
            "/node/toggle-participation",
            post(node_toggle_participation_handler),
        )
        .route("/node/logs", get(node_logs_handler))
        .route("/peers", get(peers_handler))
        .route("/v1/system/peers", get(peers_handler))
        .route("/v1/system/bootstrap", get(bootstrap_handler))
        .route("/v1/system/bootstrap", post(register_bootstrap_handler))
        .route("/ledger/head", get(ledger_head_handler))
        .route("/ledger/stats", get(ledger_stats_handler))
        .route("/ledger/export", get(ledger_export_handler))
        .route("/layers/next", get(next_layer_handler))
        .route(
            "/v1/system/scheduler-decisions",
            get(scheduler_decisions_handler),
        )
        .route(
            "/v1/system/model-rollout",
            get(model_rollout_status_handler),
        )
        .route(
            "/v1/system/model-rollout/reset-rollback",
            post(model_rollout_reset_handler),
        )
        .route("/v1/system/scout-ingress", get(scout_ingress_handler))
        .route(
            "/v1/system/scout-ingress",
            post(scout_ingress_update_handler),
        )
        .route(
            "/v1/system/scout-runtime/reset",
            post(scout_runtime_reset_handler),
        )
        .route(
            "/v1/system/latency/reset",
            post(latency_runtime_reset_handler),
        )
        .route(
            "/v1/system/speculative-trace",
            get(speculative_trace_handler),
        )
        .route(
            "/v1/system/speculative-trace/reset",
            post(speculative_trace_reset_handler),
        )
        .route("/v1/system/scout-config", get(scout_config_handler))
        .route("/network-config", get(network_config_handler))
        .route("/v1/system/network-config", get(network_config_handler))
        .route(
            "/v1/system/network-config",
            post(network_config_update_handler),
        )
        .route(
            "/browser-layer/register",
            post(browser_layer_register_handler),
        )
        .route("/browser-layer/work", get(browser_layer_work_handler))
        .route("/browser-layer/submit", post(browser_layer_submit_handler))
        .route("/credits/:wallet", get(credits_balance_handler))
        .route("/credits/tx/:tx_id", get(credits_tx_handler))
        .route("/pipeline/forward", post(pipeline_forward_handler))
        .route(
            "/pipeline/pop-forward-result",
            get(pipeline_pop_forward_result_handler),
        )
        .route("/broadcast-work", post(broadcast_work_handler))
        .route(
            "/signed/broadcast-work",
            post(signed_broadcast_work_handler),
        )
        .route("/pop-result", get(pop_result_handler))
        .route("/pop-work", get(pop_work_handler))
        .route("/v1/scout/work", get(pop_work_handler))
        .route("/submit-draft", post(submit_draft_handler))
        .route("/v1/scout/draft", post(submit_draft_handler))
        .route("/v1/scout/client-event", post(scout_client_event_handler))
        .route("/v1/telemetry/webgpu", post(webgpu_telemetry_handler))
        .route("/metrics/webgpu-coverage", get(webgpu_coverage_handler))
        .route("/v1/pow/challenge", get(pow_challenge_handler))
        .route("/v1/pow/verify", post(pow_verify_handler))
        .route("/signed/submit-draft", post(signed_submit_draft_handler))
        .route("/signed/register-node", post(signed_register_node_handler))
        .route("/signed/heartbeat", post(signed_heartbeat_handler))
        .route(
            "/signed/deregister-node",
            post(signed_deregister_node_handler),
        )
        .route(
            "/signed/metrics-report",
            post(signed_metrics_report_handler),
        )
        .route("/alerts", get(alerts_handler))
        .route(
            "/private-mesh/register",
            post(private_mesh_register_handler),
        )
        .route(
            "/private-mesh/deregister",
            post(private_mesh_deregister_handler),
        )
        .route("/private-mesh/groups", get(private_mesh_groups_handler))
        .route("/private-mesh/route", post(private_mesh_route_handler))
        .route("/admin/api-keys", post(admin_api_key_handler))
        .route("/v1/chat/completions", post(chat_completions_handler))
        .route("/ws/generate", get(ws_generate_handler))
        .route("/scout/penalty", post(scout_penalty_update_handler))
        .route("/scout/penalty", get(scout_penalty_status_handler))
        .route("/metrics", get(metrics_handler))
        .route("/v1/leaderboard", get(leaderboard_handler))
        .route("/metrics/summary", get(metrics_summary_handler))
        .route("/metrics/latency-profile", get(latency_profile_handler))
        .route("/dashboard", get(dashboard_handler))
        .layer(cors)
        .with_state(state)
}

// ─── Main ───────────────────────────────────────────────────────────────────

pub async fn run(args: Vec<String>) -> anyhow::Result<()> {
    let mut cli = Cli::parse_from(&args);
    let data = ensure_data_dir().await?;
    let config_path = std::env::var("SHARD_NODE_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| data.join("shard-node.yaml"));
    let node_cfg = NodeRuntimeConfig::load_with_env(&config_path);
    let node_role = match node_cfg.node_role {
        NodeRole::Gateway => "gateway",
        NodeRole::Shard => "shard",
        NodeRole::Scout => "scout",
    }
    .to_string();
    if node_role == "scout" {
        cli.contribute = true;
    }
    let identity_path = data.join("identity.json");

    if let Some(command) = cli.command.clone() {
        match command {
            DaemonCommand::Wallet { command } => {
                handle_wallet_command(command, &identity_path)?;
                return Ok(());
            }
            DaemonCommand::Model { command } => {
                handle_model_command(command, &data).await?;
                return Ok(());
            }
        }
    }

    if let Ok(port_from_env) = std::env::var("PORT") {
        match port_from_env.parse::<u16>() {
            Ok(port) => {
                cli.telemetry_ws_port = port;
            }
            Err(error) => {
                eprintln!("Ignoring invalid PORT environment variable ({port_from_env}): {error}");
            }
        }
    }

    if let Err(detail) = preflight_ports(&cli) {
        return Err(anyhow::anyhow!(detail));
    }
    let p2p_listen_addrs = p2p_listen_multiaddrs(&cli)?;

    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
    let filter_layer = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&cli.log_level));

    let fmt_layer = tracing_subscriber::fmt::layer().json();

    match opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(opentelemetry_otlp::new_exporter().tonic())
        .with_trace_config(opentelemetry_sdk::trace::config().with_resource(
            opentelemetry_sdk::Resource::new(vec![opentelemetry::KeyValue::new(
                "service.name",
                "shard-daemon",
            )]),
        ))
        .install_batch(opentelemetry_sdk::runtime::Tokio)
    {
        Ok(tracer) => {
            let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);
            let _ = tracing_subscriber::registry()
                .with(filter_layer)
                .with(fmt_layer)
                .with(telemetry_layer)
                .try_init();
        }
        Err(_) => {
            let _ = tracing_subscriber::registry()
                .with(filter_layer)
                .with(fmt_layer)
                .try_init();
        }
    }

    let topo_path = data.join("topology.json");
    let known_peers_path = data.join("known_peers.json");
    let bootstrap_registry_path = data.join("bootstrap_registry.json");
    let private_mode_enabled = cli.private_mode;
    let network_policy = if private_mode_enabled {
        let policy_path = std::env::var("SHARD_NETWORK_POLICY_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("network_policy.yaml"));
        if !policy_path.exists() {
            return Err(anyhow::anyhow!(
                "private mode requires network policy file at {}",
                policy_path.display()
            ));
        }
        let loaded = NetworkPolicy::from_yaml(policy_path.as_path())?;
        Some(Arc::new(loaded))
    } else {
        None
    };

    let file_bootstrap = if let Some(path) = &cli.bootstrap_file {
        read_bootstrap_file(path).await
    } else {
        Vec::new()
    };
    let persisted = load_persisted_peers(&known_peers_path).await;
    let mut loaded_bootstrap_registry = load_bootstrap_registry(&bootstrap_registry_path).await;
    let now_bootstrap_init = now_ms();
    let registry_ttl_ms = bootstrap_registry_ttl_ms();
    let registry_min_score = bootstrap_registry_min_score();
    let stale_registry_ids = prune_bootstrap_registry(
        &mut loaded_bootstrap_registry,
        now_bootstrap_init,
        registry_ttl_ms,
    );
    if !stale_registry_ids.is_empty() {
        tracing::info!(
            removed = stale_registry_ids.len(),
            ttl_ms = registry_ttl_ms,
            "pruned stale bootstrap registry entries during startup"
        );
        save_bootstrap_registry(&bootstrap_registry_path, &loaded_bootstrap_registry).await;
    }
    let registry_seed_bootstrap = bootstrap_registry_seed_addrs(
        &loaded_bootstrap_registry,
        now_bootstrap_init,
        registry_ttl_ms,
        registry_min_score,
    );

    // Optional defaults from environment to avoid stale hardcoded peers.
    let default_bootstrap = std::env::var("SHARD_DEFAULT_BOOTSTRAP")
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Fallback public bootstrap peer for the production Shard mesh.
    //
    // NOTE: This address is a convenience default so that a freshly
    // installed daemon can join the public network without any flags.
    // Operators can always override it via:
    //   - CLI:  --bootstrap-node ...
    //   - Env:  SHARD_DEFAULT_BOOTSTRAP=/ip4/.../tcp/.../p2p/...
    //
    // If the production bootstrap peer changes in the future, update
    // this multiaddr and the corresponding docs in docs/ENVIRONMENT.md.
    let hardcoded_bootstrap = vec![
        "/ip4/35.175.242.222/tcp/4001/p2p/12D3KooWPQqkkZk7NeWA2b1FeWYuBFRW8X7Q9ugymnzxeKJHFLUV"
            .to_string(),
        "/dns4/shard-bootstrap-iad-0605.fly.dev/tcp/4001/p2p/12D3KooWLc6Z4NNfWtcm9Pwu8rNhPf8UGT9bATZw2X3eEZdJpbyD"
            .to_string(),
        "/dns4/shard-bootstrap-lax-0605.fly.dev/tcp/4001/p2p/12D3KooWH67LRDPMC2oJ8rFtZD2oWd6UG52zhYZVF6fGswkYdcDF"
            .to_string(),
    ];
    let hardcoded_relay = vec![
        "/ip4/35.175.242.222/tcp/4001/p2p/12D3KooWPQqkkZk7NeWA2b1FeWYuBFRW8X7Q9ugymnzxeKJHFLUV"
            .to_string(),
    ];

    // Fetch bootstrap peers from discovery URL if configured
    let url_bootstrap = if let Some(url) = &cli.bootstrap_url {
        match bootstrap_discovery::fetch_bootstrap_peers(url).await {
            Ok(peers) => {
                tracing::info!(
                    count = peers.len(),
                    "Fetched bootstrap peers from discovery URL"
                );
                peers.into_iter().map(|p| p.multiaddr).collect()
            }
            Err(_e) => {
                tracing::warn!({});
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    let allow_private_bootstrap = std::env::var("SHARD_ALLOW_PRIVATE_BOOTSTRAP")
        .ok()
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false);
    let allow_private_relay = std::env::var("SHARD_ALLOW_PRIVATE_RELAY")
        .ok()
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false);

    let mut bootstrap_sources = default_bootstrap;
    bootstrap_sources.extend(cli.bootstrap_node.clone());
    bootstrap_sources.extend(file_bootstrap);
    bootstrap_sources.extend(persisted);
    bootstrap_sources.extend(url_bootstrap);
    bootstrap_sources.extend(registry_seed_bootstrap.clone());
    if private_mode_enabled {
        bootstrap_sources.clear();
        if let Some(policy) = network_policy.as_ref() {
            bootstrap_sources.extend(policy.allowed_bootstrap_addrs.clone());
        }
    }

    let hardcoded_mode =
        parse_hardcoded_bootstrap_mode(std::env::var("SHARD_HARDCODED_BOOTSTRAP_MODE").ok());
    let include_hardcoded =
        should_include_hardcoded_bootstrap(hardcoded_mode, !bootstrap_sources.is_empty());
    if include_hardcoded && !private_mode_enabled {
        bootstrap_sources.extend(hardcoded_bootstrap);
    }

    let bootstrap_addrs = filter_bootstrap_addrs(bootstrap_sources, allow_private_bootstrap);

    let relay_sources = std::env::var("SHARD_RELAY_BOOTSTRAP")
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let default_relay = std::env::var("SHARD_DEFAULT_RELAY")
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut relay_addrs = default_relay;
    relay_addrs.extend(relay_sources);
    if relay_addrs.is_empty() && include_hardcoded && !private_mode_enabled {
        relay_addrs.extend(hardcoded_relay);
    }
    if private_mode_enabled {
        relay_addrs.clear();
    }
    relay_addrs = filter_relay_addrs(relay_addrs, allow_private_relay);
    tracing::info!(
        bootstrap_count = bootstrap_addrs.len(),
        relay_bootstrap_count = relay_addrs.len(),
        registry_bootstrap_count = registry_seed_bootstrap.len(),
        bootstrap_registry_entries = loaded_bootstrap_registry.len(),
        bootstrap_registry_ttl_ms = registry_ttl_ms,
        bootstrap_registry_min_score = registry_min_score,
        include_hardcoded_bootstrap = include_hardcoded,
        hardcoded_bootstrap_mode = ?hardcoded_mode,
        "resolved bootstrap peers"
    );

    // ── channels ──
    let (work_tx, mut work_rx) = mpsc::channel::<WorkRequest>(256);
    let (pipeline_tx, mut pipeline_rx) = mpsc::channel::<PipelineDispatch>(256);
    let (browser_result_tx, mut browser_result_rx) = mpsc::channel::<ForwardPassActivation>(256);
    let (scout_draft_tx, scout_draft_rx) = mpsc::channel::<ScoutDraft>(64);
    let (ban_tx, mut ban_rx) = mpsc::channel::<(String, String)>(128);
    let (draft_publish_tx, mut draft_publish_rx) = mpsc::channel::<WorkResponse>(64);
    let canary_rollout_cfg = CanaryRolloutConfig::from_env(cli.model_id.as_str());
    let bootstrap_ring_path = std::env::var("SHARD_BOOTSTRAP_RING_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("deploy/config/bootstrap-ring.yaml"));
    let bootstrap_ring_strict = std::env::var("SHARD_BOOTSTRAP_RING_STRICT")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(true);
    let bootstrap_refuse_work_override = std::env::var("SHARD_BOOTSTRAP_REFUSE_WORK_BELOW_MIN")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        });
    let mut bootstrap_ring = if bootstrap_ring_path.exists() {
        match bootstrap_ring::BootstrapRing::from_config(bootstrap_ring_path.as_path()) {
            Ok(ring) => Some(ring),
            Err(error) => {
                if bootstrap_ring_strict {
                    return Err(anyhow::anyhow!(
                        "failed loading bootstrap ring config at {}: {}",
                        bootstrap_ring_path.display(),
                        error
                    ));
                }
                tracing::warn!(
                    error = %error,
                    path = %bootstrap_ring_path.display(),
                    "failed loading bootstrap ring config; continuing without ring health gate"
                );
                None
            }
        }
    } else {
        None
    };
    if let Some(ring) = bootstrap_ring.as_mut() {
        if let Some(override_refuse_work) = bootstrap_refuse_work_override {
            ring.refuse_work_below_min_bootstrap = override_refuse_work;
            tracing::info!(
                override_refuse_work,
                "applied bootstrap ring refuse-work override"
            );
        }
        ring.connect_all();
    }
    let bootstrap_ring = bootstrap_ring.map(Arc::new);

    let node_identity = NodeIdentity::load_or_create(&identity_path)?;
    let node_wallet = node_identity.wallet_address();
    let signing_key = node_identity.signing_key().clone();
    let id_keys = node_identity.libp2p_keypair()?;
    let local_peer_id = PeerId::from(id_keys.public());
    let (consensus_handle, consensus_in_tx, mut consensus_out_rx) = if cli.ha_mode {
        let config = LeaderElectionConfig::from_config(&config_path);
        let (handle, out_rx) =
            consensus::leader::spawn_leader_election(local_peer_id.to_string(), config);
        tracing::info!("HA mode enabled - leader election active");
        let sender = handle.input_sender();
        (Some(Arc::new(handle)), Some(sender), Some(out_rx))
    } else {
        (None, None, None)
    };
    let ledger_store = Arc::new(LedgerStore::new(&data));
    let loaded_ledger = ledger_store.load_or_init()?;
    let node_reputation_path = data.join("node_reputation.json");
    let loaded_reputation = load_reputation(&node_reputation_path);
    let metrics_persistence = Arc::new(resolve_metrics_persistence(&data));
    metrics_persistence.initialize().await?;
    let heartbeat_timeout_ms = std::env::var("SHARD_HEARTBEAT_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse::<u128>().ok())
        .unwrap_or(30_000);

    let initial_api_keys: HashSet<String> = std::env::var("SHARD_API_KEYS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    let admin_key = std::env::var("SHARD_ADMIN_KEY").ok();
    let require_api_key = std::env::var("SHARD_REQUIRE_API_KEY")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);

    let initial_credit_nonce = loaded_ledger.head().height.saturating_add(1);
    let state = SharedState {
        topology: Arc::new(Mutex::new(TopologyState {
            local_peer_id: local_peer_id.to_string(),
            listen_addrs: Vec::new(),
            webrtc_addr: None,
            quic_addr: None,
            ws_addr: None,
            public_api_addr: cli.public_host.clone(),
            is_public: cli.public_api,
            relay_server_enabled: cli.relay_mode,
            relay_reservation_active: false,
            nat_status: "unknown".to_string(),
            contribute_enabled: cli.contribute,
            capacity: 100, // Default: 100 tokens/sec
            load: 0,
            latency_ms: 0.0,
        })),
        peers: Arc::new(Mutex::new(HashMap::new())),
        known_peers: Arc::new(Mutex::new(bootstrap_addrs.clone())),
        known_peers_path: known_peers_path.clone(),
        results: Arc::new(Mutex::new(VecDeque::new())),
        scout_work: Arc::new(Mutex::new(VecDeque::new())),
        work_tx,
        pipeline_tx,
        browser_result_tx,
        daemon_start: now_ms(),
        capacity: Arc::new(AtomicU32::new(100)), // Default: 100 tokens/sec
        current_load: Arc::new(AtomicU32::new(0)),
        avg_latency_ms: Arc::new(AtomicU32::new(0)),
        fast_verifier_bypass_until_ms: Arc::new(AtomicU64::new(0)),
        avg_draft_arrival_ms: Arc::new(AtomicU32::new(0)),
        avg_accepted_tokens_x100: Arc::new(AtomicU32::new(0)),
        gossipsub_latency_hist: Arc::new(LatencyHistogram::new()),
        credit_nonce: Arc::new(AtomicU64::new(initial_credit_nonce)),
        scout_penalties: Arc::new(Mutex::new(ScoutPenaltyBook::default())),
        backward_passes: Arc::new(Mutex::new(VecDeque::new())),
        layer_routes: Arc::new(Mutex::new(LayerRoutingTable::default())),
        race_router: Arc::new(Mutex::new(RaceRouter::default())),
        ledger: Arc::new(Mutex::new(loaded_ledger)),
        ledger_store,
        browser_sessions: Arc::new(Mutex::new(HashMap::new())),
        scout_client_runtime: Arc::new(Mutex::new(HashMap::new())),
        scout_work_last_poll: Arc::new(Mutex::new(HashMap::new())),
        scout_draft_last_submit: Arc::new(Mutex::new(HashMap::new())),
        scout_work_leases: Arc::new(Mutex::new(HashMap::new())),
        scout_blackout: Arc::new(Mutex::new(ScoutBlackoutState::default())),
        webgpu_stats: Arc::new(Mutex::new(WebGPUStats::default())),
        browser_work: Arc::new(Mutex::new(VecDeque::new())),
        node_wallet: node_wallet.clone(),
        model_id: cli.model_id.clone(),
        layer_start: cli.layer_start,
        layer_end: cli.layer_end,
        race_pool_size: cli.race_pool_size,
        race_timeout_ms: cli.race_timeout_ms,
        engine: Arc::new(Mutex::new(None)),
        replay_nonces: Arc::new(Mutex::new(HashMap::new())),
        system_metrics: Arc::new(SystemMetrics::default()),
        node_metric_reports: Arc::new(Mutex::new(HashMap::new())),
        metrics_persistence,
        heartbeat_timeout_ms,
        idempotent_results: Arc::new(Mutex::new(HashMap::new())),
        node_reputation: Arc::new(Mutex::new(loaded_reputation)),
        node_reputation_path,
        node_role,
        participation_enabled: Arc::new(AtomicBool::new(true)),
        resource_policy: ResourcePolicy {
            max_cpu_usage: node_cfg.max_cpu,
            max_gpu_usage: node_cfg.max_gpu,
            idle_only_mode: node_cfg.idle_only,
            load_threshold_cutoff: node_cfg.load_threshold_cutoff,
        },
        event_log: Arc::new(Mutex::new(VecDeque::new())),
        node_public_key: node_wallet.clone(),
        heartbeat_interval_seconds: Arc::new(AtomicU64::new(
            node_cfg.heartbeat_interval_seconds.clamp(2, 300),
        )),
        scout_timeout_ms: Arc::new(AtomicU64::new(env_live_scout_timeout_ms())),
        max_scouts: Arc::new(AtomicUsize::new(env_live_max_scouts())),
        acceptance_threshold_bps: Arc::new(AtomicU64::new(
            env_live_acceptance_threshold_bps(),
        )),
        public_host: cli
            .public_host
            .clone()
            .and_then(|h| normalize_public_host(&h)),
        tcp_port: cli.tcp_port,
        webrtc_port: cli.webrtc_port,
        quic_port: cli.quic_port,
        active_requests: Arc::new(Mutex::new(HashMap::new())),
        fallback_config: Arc::new(FallbackConfig::from_env()),
        envelope_verifier: Arc::new(Mutex::new(EnvelopeVerifier::with_defaults())),
        pow_manager: Arc::new(Mutex::new(PowChallengeManager::with_defaults())),
        private_mesh: Arc::new(Mutex::new(PrivateMeshRegistry::new())),
        alert_manager: Arc::new(Mutex::new(AlertManager::new())),
        api_keys: Arc::new(Mutex::new(initial_api_keys)),
        rate_limiter: Arc::new(shard_gateway::rate_limiter::RateLimiter::new(
            shard_gateway::rate_limiter::RateLimitConfig::default(),
        )),
        admin_key,
        require_api_key,
        signing_key: signing_key.clone(),
        ice_servers: Arc::new(Mutex::new(if cli.ice_servers.is_empty() {
            node_cfg.ice_servers
        } else {
            cli.ice_servers
        })),
        scout_draft_tx,
        scout_draft_rx: Arc::new(Mutex::new(Some(scout_draft_rx))),
        scout_draft_mailbox: Arc::new(Mutex::new(HashMap::new())),
        scout_draft_notifiers: Arc::new(Mutex::new(HashMap::new())),
        speculative_pending: Arc::new(Mutex::new(HashMap::new())),
        speculative_terminal: Arc::new(Mutex::new(HashMap::new())),
        speculative_trace: Arc::new(Mutex::new(VecDeque::new())),
        draft_buffers: Arc::new(Mutex::new(HashMap::new())),
        ban_tx,
        draft_publish_tx,
        scout_timeout_tracker: Arc::new(Mutex::new(ScoutTimeoutTracker::new())),
        bootstrap_failures: Arc::new(Mutex::new(HashMap::new())),
        bootstrap_ring,
        network_policy: network_policy.clone(),
        private_mode: private_mode_enabled,
        bootstrap_registry: Arc::new(Mutex::new(loaded_bootstrap_registry)),
        bootstrap_registry_path,
        scheduler_decisions: Arc::new(Mutex::new(VecDeque::new())),
        mesh_probe_backoff: Arc::new(Mutex::new(HashMap::new())),
        canary_rollout: Arc::new(Mutex::new(CanaryRolloutController::new(
            cli.model_id.clone(),
            canary_rollout_cfg,
        ))),
        scout_ingress_enabled: Arc::new(AtomicBool::new(true)),
        shutdown: Arc::new(AtomicBool::new(false)),
        in_flight_count: Arc::new(AtomicUsize::new(0)),
        consensus: consensus_handle,
    };

    #[cfg(target_os = "windows")]
    const LIB_NAME: &str = "shard_engine.dll";
    #[cfg(target_os = "macos")]
    const LIB_NAME: &str = "libshard_engine.dylib";
    #[cfg(target_os = "linux")]
    const LIB_NAME: &str = "libshard_engine.so";

    let lib_path_opt = std::env::var("BITNET_LIB").or_else(|_| {
        let local_path = std::path::PathBuf::from(LIB_NAME);
        if local_path.exists() {
            Ok(local_path.to_string_lossy().into_owned())
        } else {
            let nested_path = std::path::PathBuf::from("bitnet").join(LIB_NAME);
            if nested_path.exists() {
                Ok(nested_path.to_string_lossy().into_owned())
            } else {
                Err(std::env::VarError::NotPresent)
            }
        }
    });

    if let (Ok(lib_path), Ok(model_path)) = (lib_path_opt, std::env::var("BITNET_MODEL")) {
        let model_c = std::ffi::CString::new(model_path).unwrap();
        let model_id_c = std::ffi::CString::new(cli.model_id.clone()).unwrap();
        let config = shard_verifier::inference::ShardInitConfig {
            model_path: model_c.as_ptr(),
            layer_start: cli.layer_start as std::ffi::c_int,
            layer_end: if cli.layer_end == 0 {
                -1
            } else {
                cli.layer_end as std::ffi::c_int
            },
            model_id: model_id_c.as_ptr(),
            pipeline_mode: 0,
        };
        match shard_verifier::inference::ShardEngine::load(&lib_path, &config) {
            Ok(engine) => {
                tracing::info!("Loaded ShardEngine from {}", lib_path);
                *state.engine.lock().await = Some(engine);
            }
            Err(err) => {
                tracing::warn!(%err, "Failed to load ShardEngine");
            }
        }
    } else {
        // Fallback or explicit auto-detect logic can be implemented here later
        tracing::debug!("BITNET_LIB or BITNET_MODEL not set, ShardEngine unavailable");
    }

    let require_engine_for_contribute = std::env::var("SHARD_REQUIRE_ENGINE_FOR_CONTRIBUTE")
        .ok()
        .map(|v| {
            let lowered = v.trim().to_ascii_lowercase();
            !matches!(lowered.as_str(), "0" | "false" | "no" | "off")
        })
        .unwrap_or(true);
    if cli.contribute && require_engine_for_contribute {
        let engine_loaded = state.engine.lock().await.is_some();
        if !engine_loaded {
            anyhow::bail!(
                "contribute mode requires a compatible engine. Ensure BITNET_LIB points to a shard_engine library exporting shard_init_ex and BITNET_MODEL points to a valid model file"
            );
        }
    }

    {
        let model_id = cli.model_id.clone();
        let data_dir = data.clone();
        tokio::spawn(async move {
            let current_version =
                std::env::var("SHARD_MODEL_VERSION").unwrap_or_else(|_| "0.0.0".to_string());
            let Ok(manifest) = fetch_model_manifest().await else {
                return;
            };
            if let Some(remote) = manifest.models.into_iter().find(|m| m.id == model_id) {
                let local_path = model_local_path(data_dir.as_path(), &remote);
                if local_path.exists()
                    && is_newer_version(&current_version, remote.version.as_str())
                {
                    tracing::warn!(
                        "model update available: {} {} -> {}. Run 'shard-daemon model pull {}' to update.",
                        remote.id,
                        current_version,
                        remote.version,
                        remote.id
                    );
                }
            }
        });
    }

    // ── build swarm ──
    // ── build transport ──
    let (relay_transport, relay_client_behaviour) = relay::client::new(local_peer_id);

    let tcp_config = libp2p::tcp::Config::default().nodelay(true);
    let dns_tcp = libp2p::dns::tokio::Transport::system(libp2p::tcp::tokio::Transport::new(
        tcp_config.clone(),
    ))?;
    let ws_dns_tcp = libp2p::websocket::Config::new(libp2p::dns::tokio::Transport::system(
        libp2p::tcp::tokio::Transport::new(tcp_config),
    )?);

    let tcp_ws = libp2p::core::transport::OrTransport::new(dns_tcp, ws_dns_tcp);
    let tcp_ws_relay = libp2p::core::transport::OrTransport::new(tcp_ws, relay_transport);

    let authenticated_transport = tcp_ws_relay
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(libp2p::noise::Config::new(&id_keys).expect("Noise config failed"))
        .multiplex(libp2p::yamux::Config::default());

    let webrtc_cert = libp2p_webrtc::tokio::Certificate::generate(&mut rand::thread_rng())?;
    let webrtc = libp2p_webrtc::tokio::Transport::new(id_keys.clone(), webrtc_cert);
    let quic = libp2p::quic::tokio::Transport::new(libp2p::quic::Config::new(&id_keys));

    use libp2p::Transport;
    let transport = authenticated_transport
        .or_transport(webrtc)
        .or_transport(quic)
        .map(|either, _| match either {
            libp2p::futures::future::Either::Left(left) => match left {
                libp2p::futures::future::Either::Left((peer_id, muxer)) => {
                    (peer_id, libp2p::core::muxing::StreamMuxerBox::new(muxer))
                }
                libp2p::futures::future::Either::Right((peer_id, muxer)) => {
                    (peer_id, libp2p::core::muxing::StreamMuxerBox::new(muxer))
                }
            },
            libp2p::futures::future::Either::Right((peer_id, muxer)) => {
                (peer_id, libp2p::core::muxing::StreamMuxerBox::new(muxer))
            }
        })
        .boxed();

    // ── build swarm ──
    let behaviour = {
        // ── Gossipsub config tuned for scalable meshes ──
        let parse_mesh = |name: &str, default: usize, min: usize, max: usize| -> usize {
            std::env::var(name)
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .map(|v| v.clamp(min, max))
                .unwrap_or(default)
        };
        let profile = std::env::var("SHARD_GOSSIPSUB_PROFILE")
            .unwrap_or_else(|_| "large".to_string())
            .to_lowercase();
        let (default_low, default_n, default_high, default_lazy) = match profile.as_str() {
            "small" => (1, 1, 2, 1),
            "medium" => (3, 5, 8, 4),
            _ => (6, 10, 20, 6),
        };
        let mesh_n_low = parse_mesh("SHARD_GOSSIPSUB_MESH_N_LOW", default_low, 1, 128);
        let mut mesh_n = parse_mesh("SHARD_GOSSIPSUB_MESH_N", default_n, 1, 128);
        let mut mesh_n_high = parse_mesh("SHARD_GOSSIPSUB_MESH_N_HIGH", default_high, 1, 256);
        if mesh_n < mesh_n_low {
            mesh_n = mesh_n_low;
        }
        if mesh_n_high < mesh_n {
            mesh_n_high = mesh_n;
        }
        let gossip_lazy = parse_mesh("SHARD_GOSSIPSUB_GOSSIP_LAZY", default_lazy, 1, 128);
        let max_transmit_size = std::env::var("SHARD_GOSSIPSUB_MAX_TRANSMIT_BYTES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .map(|v| v.clamp(64 * 1024, 2 * 1024 * 1024))
            .unwrap_or(512 * 1024);

        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .mesh_n_low(mesh_n_low)
            .mesh_n(mesh_n)
            .mesh_n_high(mesh_n_high)
            .gossip_lazy(gossip_lazy)
            .heartbeat_interval(Duration::from_secs(1))
            .max_transmit_size(max_transmit_size)
            .validation_mode(gossipsub::ValidationMode::Permissive)
            .build()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(id_keys.clone()),
            gossipsub_config,
        )
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let mut kad = KadBehaviour::new(local_peer_id, MemoryStore::new(local_peer_id));
        // Public nodes participate as DHT servers (store + serve records).
        // Private/NAT'd nodes use Client mode (query only).
        if cli.relay_mode || cli.public_api {
            kad.set_mode(Some(libp2p::kad::Mode::Server));
        } else {
            kad.set_mode(Some(libp2p::kad::Mode::Client));
        }
        let handshake = request_response::cbor::Behaviour::new(
            [(
                StreamProtocol::new("/shard/1.0.0/handshake"),
                ProtocolSupport::Full,
            )],
            request_response::Config::default(),
        );
        let verify = request_response::cbor::Behaviour::new(
            [(
                StreamProtocol::new("/shard/shard/verify/1.0.0"),
                ProtocolSupport::Full,
            )],
            request_response::Config::default(),
        );
        let control_work = request_response::cbor::Behaviour::new(
            [(
                StreamProtocol::new("/shard/control/work/1.0.0"),
                ProtocolSupport::Full,
            )],
            request_response::Config::default(),
        );
        let ledger_sync = request_response::cbor::Behaviour::new(
            [(
                StreamProtocol::new("/shard/ledger/sync/1.0.0"),
                ProtocolSupport::Full,
            )],
            request_response::Config::default(),
        );
        let relay_server = if cli.relay_mode {
            libp2p::swarm::behaviour::toggle::Toggle::from(Some(relay::Behaviour::new(
                local_peer_id,
                Default::default(),
            )))
        } else {
            libp2p::swarm::behaviour::toggle::Toggle::from(None)
        };
        let relay_client = if !cli.relay_mode {
            libp2p::swarm::behaviour::toggle::Toggle::from(Some(relay_client_behaviour))
        } else {
            libp2p::swarm::behaviour::toggle::Toggle::from(None)
        };
        let dcutr = if !cli.relay_mode && cli.nat_traversal {
            libp2p::swarm::behaviour::toggle::Toggle::from(Some(dcutr::Behaviour::new(
                local_peer_id,
            )))
        } else {
            libp2p::swarm::behaviour::toggle::Toggle::from(None)
        };
        let autonat = autonat::v1::Behaviour::new(local_peer_id, build_autonat_config());
        let identify = identify::Behaviour::new(identify::Config::new(
            "/shard/1.0.0".to_string(),
            id_keys.public(),
        ));
        let ping = ping::Behaviour::new(ping::Config::new());
        let mdns =
            libp2p::mdns::tokio::Behaviour::new(libp2p::mdns::Config::default(), local_peer_id)
                .map_err(|e| anyhow::anyhow!(e))?;
        ShardBehaviour {
            gossipsub,
            kad,
            handshake,
            verify,
            control_work,
            ledger_sync,
            relay_server,
            relay_client,
            dcutr,
            autonat,
            identify,
            ping,
            mdns,
        }
    };

    let mut swarm = libp2p::Swarm::new(
        transport,
        behaviour,
        local_peer_id,
        libp2p::swarm::Config::with_tokio_executor(),
    );

    // ── gossipsub topics ──
    let work_topic = IdentTopic::new("shard-work");
    let result_topic = IdentTopic::new("shard-work-result");
    let forward_topic = IdentTopic::new("shard-forward-pass");
    let forward_result_topic = IdentTopic::new("shard-forward-result");
    let backward_topic = IdentTopic::new("shard-backward-pass");
    let ledger_topic = IdentTopic::new("shard-ledger-tx");
    let layer_announce_topic = IdentTopic::new("shard-layer-announcements");
    let bootstrap_topic = IdentTopic::new("shard-bootstrap");
    let auction_topic = IdentTopic::new("auction.prompt");
    let ban_topic = IdentTopic::new("shard-ban-list");
    let election_topic = IdentTopic::new("shard/election/v1");

    swarm.behaviour_mut().gossipsub.subscribe(&work_topic)?;
    swarm.behaviour_mut().gossipsub.subscribe(&result_topic)?;
    swarm.behaviour_mut().gossipsub.subscribe(&forward_topic)?;
    swarm
        .behaviour_mut()
        .gossipsub
        .subscribe(&forward_result_topic)?;
    swarm.behaviour_mut().gossipsub.subscribe(&backward_topic)?;
    swarm.behaviour_mut().gossipsub.subscribe(&ledger_topic)?;
    swarm
        .behaviour_mut()
        .gossipsub
        .subscribe(&layer_announce_topic)?;
    swarm.behaviour_mut().gossipsub.subscribe(&bootstrap_topic)?;
    swarm.behaviour_mut().gossipsub.subscribe(&auction_topic)?;
    swarm.behaviour_mut().gossipsub.subscribe(&ban_topic)?;
    if cli.ha_mode {
        swarm.behaviour_mut().gossipsub.subscribe(&election_topic)?;
    }

    // ── listen addresses ──
    for addr in p2p_listen_addrs {
        swarm.listen_on(addr)?;
    }
    if should_enable_ipv6_p2p_listeners() {
        for addr_str in [
            format!("/ip6/::/tcp/{}", cli.tcp_port),
            format!("/ip6/::/tcp/{}/ws", cli.tcp_port + 100),
            format!("/ip6/::/udp/{}/webrtc-direct", cli.webrtc_port),
            format!("/ip6/::/udp/{}/quic-v1", cli.quic_port),
        ] {
            match addr_str.parse::<Multiaddr>() {
                Ok(addr) => {
                    if let Err(error) = swarm.listen_on(addr.clone()) {
                        tracing::warn!(%addr, %error, "failed to bind optional IPv6 p2p listener");
                    }
                }
                Err(error) => {
                    tracing::warn!(%addr_str, %error, "failed to parse optional IPv6 p2p listener");
                }
            }
        }
    }

    // ── bootstrap peers ──
    for addr_str in &bootstrap_addrs {
        if let Ok(addr) = addr_str.parse::<Multiaddr>() {
            if let Some(peer_id) = extract_peer_id_from_multiaddr(&addr) {
                tracing::info!(%peer_id, "dialing bootstrap peer");
            } else {
                tracing::info!("dialing bootstrap peer");
            }
            let _ = swarm.dial(addr.clone());
            if let Some(peer_id) = extract_peer_id_from_multiaddr(&addr) {
                swarm.behaviour_mut().kad.add_address(&peer_id, addr);
            }
        }
    }

    if !cli.relay_mode && cli.nat_traversal && !relay_addrs.is_empty() {
        for addr_str in &relay_addrs {
            if let Ok(mut addr) = addr_str.parse::<Multiaddr>() {
                if let Some(peer_id) = extract_peer_id_from_multiaddr(&addr) {
                    tracing::info!(%peer_id, "dialing relay bootstrap peer");
                }
                let _ = swarm.dial(addr.clone());
                if !addr
                    .iter()
                    .any(|p| matches!(p, libp2p::multiaddr::Protocol::P2p(_)))
                {
                    continue;
                }
                addr.push(libp2p::multiaddr::Protocol::P2pCircuit);
                tracing::info!(%addr, "attempting relay reservation");
                let _ = swarm.listen_on(addr);
            }
        }
    }

    if !bootstrap_addrs.is_empty() {
        if let Err(e) = swarm.behaviour_mut().kad.bootstrap() {
            tracing::warn!(%e, "failed to bootstrap Kademlia DHT");
        }
    }

    // Advertise hosted layer start in Kademlia DHT provider index.
    let local_layer_key = provider_key(&cli.model_id, cli.layer_start);
    if let Err(e) = swarm.behaviour_mut().kad.start_providing(local_layer_key) {
        tracing::warn!(%e, "failed to publish local layer provider record");
    }

    telemetry_ws::spawn_telemetry_ws_server(state.clone(), cli.telemetry_ws_port, cli.public_api);

    // ── spawn ICE server refresh loop ──
    if let Some(url) = cli.ice_provider_url {
        let refresh_state = state.clone();
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            loop {
                match client.get(&url).send().await {
                    Ok(resp) => {
                        if let Ok(new_servers) = resp.json::<Vec<String>>().await {
                            tracing::info!(
                                count = new_servers.len(),
                                "refreshed ICE servers from provider"
                            );
                            *refresh_state.ice_servers.lock().await = new_servers;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(%e, "failed to refresh ICE servers from provider");
                    }
                }
                tokio::time::sleep(Duration::from_secs(3600)).await; // Refresh every hour
            }
        });
    }

    // ── spawn bootstrap advertisement loop ──
    if let Some(advertise_url) = cli.bootstrap_advertise_url {
        let advertise_state = state.clone();
        let local_peer_id = local_peer_id.to_string();
        let stability_threshold = cli.stability_threshold_hours;
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(300)).await; // Check every 5 minutes

                let now = now_ms();
                let uptime_hours = (now - advertise_state.daemon_start) / (1000 * 60 * 60);
                let uptime_hours_u64 = uptime_hours as u64;

                // Only advertise if stable enough
                if uptime_hours_u64 < stability_threshold {
                    continue;
                }

                // Get our advertised addresses
                let topo = advertise_state.topology.lock().await;
                let addrs = topo.listen_addrs.clone();
                let public_api_addr = advertise_state
                    .public_host
                    .clone()
                    .or(topo.public_api_addr.clone());
                drop(topo);

                if let Some(multiaddr) = addrs.first() {
                    let capability_tier = node_capability_tier(
                        advertise_state.node_role.as_str(),
                        advertise_state.resource_policy.max_gpu_usage,
                        cli.relay_mode,
                        cli.public_api,
                    );
                    let registration = bootstrap_discovery::BootstrapRegistration {
                        peer_id: local_peer_id.clone(),
                        multiaddr: multiaddr.clone(),
                        stability_score: 100, // TODO: calculate dynamically
                        uptime_hours: uptime_hours_u64,
                        version: env!("CARGO_PKG_VERSION").to_string(),
                        role: Some(advertise_state.node_role.clone()),
                        capability_tier: Some(capability_tier.clone()),
                        gpu_available: Some(node_gpu_available_for_tier(capability_tier.as_str())),
                        accepts_scout_work: Some(node_accepts_scout_work(
                            advertise_state.node_role.as_str(),
                            advertise_state.participation_enabled.load(Ordering::Relaxed),
                            cli.relay_mode,
                        )),
                        public_api: Some(cli.public_api),
                        public_api_addr,
                    };

                    if let Err(_e) =
                        bootstrap_discovery::register_as_bootstrap(&advertise_url, &registration)
                            .await
                    {
                        tracing::warn!({});
                    } else {
                        tracing::info!(peer_id = %local_peer_id, hours = uptime_hours, "Registered as bootstrap peer");
                    }
                }
            }
        });
    }

    // ── spawn HTTP control-plane server ──
    let http_state = state.clone();
    let control_bind_ip = if cli.public_api {
        [0, 0, 0, 0]
    } else {
        [127, 0, 0, 1]
    };
    let control_addr = SocketAddr::from((control_bind_ip, cli.control_port));
    let listener = tokio::net::TcpListener::bind(control_addr)
        .await
        .expect("failed to bind control-plane port");
    let control_port = listener
        .local_addr()
        .map(|addr| addr.port())
        .unwrap_or(cli.control_port);
    let public_host = cli.public_host.as_deref().and_then(normalize_public_host);
    let cors_origins = build_cors_origins(control_port, public_host.as_deref());
    let allowed_origin_set = cors_origins
        .iter()
        .filter_map(|value| value.to_str().ok())
        .map(|value| value.to_string())
        .collect::<HashSet<_>>();
    let policy = ApiAccessPolicy {
        allowed_origins: Arc::new(allowed_origin_set),
        public_api: cli.public_api,
    };
    println!("control_port={control_port}");

    tokio::spawn(async move {
        let app = create_router(http_state, cors_origins, policy);
        let addr = SocketAddr::from((control_bind_ip, control_port));
        tracing::info!(%addr, "control-plane HTTP server starting");
        axum::serve(listener, app)
            .await
            .expect("control-plane server crashed");
    });

    // Signed self-registration and heartbeat loop for controlled clusters.
    let heartbeat_state = state.clone();
    let heartbeat_signing_key = signing_key.clone();
    tokio::spawn(async move {
        let mut nonce = 1u64;
        let pubkey = hex::encode(heartbeat_signing_key.verifying_key().to_bytes());
        let role = heartbeat_state.node_role.clone();
        let register = SignedEnvelope::sign(
            NodeRegistration {
                node_pubkey: pubkey.clone(),
                role: role.clone(),
                capacity: Some(heartbeat_state.capacity.load(Ordering::Relaxed) as u64),
                timestamp_ms: Some(now_ms()),
            },
            &heartbeat_signing_key,
            nonce,
            now_ms(),
        );
        nonce = nonce.saturating_add(1);
        if register.verify().is_ok()
            && accept_replay_nonce(
                &heartbeat_state.replay_nonces,
                &register.signer_pubkey_hex,
                register.nonce,
            )
            .await
        {
            upsert_node_snapshot(
                &heartbeat_state,
                pubkey.clone(),
                role.clone(),
                0,
                0,
                0,
                None,
                None,
                None,
                None,
                now_ms(),
            )
            .await;
            append_event_log(
                &heartbeat_state,
                format!("signed registration accepted for {}", pubkey),
            )
            .await;
        }

        loop {
            let interval_secs = heartbeat_state
                .heartbeat_interval_seconds
                .load(Ordering::Relaxed)
                .clamp(2, 300);
            tokio::time::sleep(Duration::from_secs(interval_secs)).await;
            let capability_tier = node_capability_tier(
                heartbeat_state.node_role.as_str(),
                heartbeat_state.resource_policy.max_gpu_usage,
                cli.relay_mode,
                cli.public_api,
            );
            let public_api_addr = {
                let topo = heartbeat_state.topology.lock().await;
                heartbeat_state
                    .public_host
                    .clone()
                    .or(topo.public_api_addr.clone())
            };
            let heartbeat = SignedEnvelope::sign(
                NodeHeartbeat {
                    node_pubkey: pubkey.clone(),
                    role: role.clone(),
                    queue_depth: heartbeat_state.in_flight_count.load(Ordering::Relaxed) as u64,
                    node_latency_ms: heartbeat_state.avg_latency_ms.load(Ordering::Relaxed) as u64,
                    uptime_seconds: ((now_ms().saturating_sub(heartbeat_state.daemon_start)) / 1000)
                        as u64,
                    capability_tier: Some(capability_tier.clone()),
                    gpu_available: Some(node_gpu_available_for_tier(capability_tier.as_str())),
                    accepts_scout_work: Some(node_accepts_scout_work(
                        heartbeat_state.node_role.as_str(),
                        heartbeat_state.participation_enabled.load(Ordering::Relaxed),
                        cli.relay_mode,
                    )),
                    public_api: Some(cli.public_api),
                    public_api_addr,
                    timestamp_ms: Some(now_ms()),
                },
                &heartbeat_signing_key,
                nonce,
                now_ms(),
            );
            nonce = nonce.saturating_add(1);
            if heartbeat.verify().is_err() {
                continue;
            }
            if !accept_replay_nonce(
                &heartbeat_state.replay_nonces,
                &heartbeat.signer_pubkey_hex,
                heartbeat.nonce,
            )
            .await
            {
                continue;
            }
            let payload = heartbeat.payload;
            upsert_node_snapshot(
                &heartbeat_state,
                payload.node_pubkey,
                payload.role,
                payload.queue_depth,
                payload.node_latency_ms,
                payload.uptime_seconds,
                payload.capability_tier,
                payload.gpu_available,
                payload.accepts_scout_work,
                payload.public_api,
                payload.timestamp_ms.unwrap_or_else(now_ms),
            )
            .await;
        }
    });

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);

    // Graceful signed deregistration signal handler.
    let shutdown_state = state.clone();
    let shutdown_signing_key = signing_key.clone();
    let shutdown_trigger = shutdown_tx.clone();
    tokio::spawn(async move {
        if shutdown_signal().await {
            shutdown_state.shutdown.store(true, Ordering::Relaxed);
            let envelope = SignedEnvelope::sign(
                NodeRegistration {
                    node_pubkey: shutdown_state.node_public_key.clone(),
                    role: shutdown_state.node_role.clone(),
                    capacity: Some(shutdown_state.capacity.load(Ordering::Relaxed) as u64),
                    timestamp_ms: Some(now_ms()),
                },
                &shutdown_signing_key,
                u64::MAX - 1,
                now_ms(),
            );
            if envelope.verify().is_ok() {
                let mut reports = shutdown_state.node_metric_reports.lock().await;
                reports.remove(shutdown_state.node_public_key.as_str());
                append_event_log(
                    &shutdown_state,
                    format!(
                        "signed deregistration for {}",
                        shutdown_state.node_public_key
                    ),
                )
                .await;
            }
            let _ = shutdown_trigger.send(true);
        }
    });

    println!();
    let control_host = if cli.public_api {
        "0.0.0.0"
    } else {
        "127.0.0.1"
    };
    println!("  ╔══════════════════════════════════════════════╗");
    println!(
        "  ║       Shard Daemon  v{}           ║",
        env!("CARGO_PKG_VERSION")
    );
    println!("  ╠══════════════════════════════════════════════╣");
    println!(
        "  ║  Peer ID      : {}…  ║",
        &local_peer_id.to_string()[..20]
    );
    println!(
        "  ║  Control API  : http://{}:{}          ║",
        control_host, control_port
    );
    println!(
        "  ║  Telemetry WS : ws://{}:{}/telemetry/ws ║",
        control_host, cli.telemetry_ws_port
    );
    println!(
        "  ║  TCP          : /ip4/0.0.0.0/tcp/{}        ║",
        cli.tcp_port
    );
    println!(
        "  ║  WebSocket    : /ip4/0.0.0.0/tcp/{}/ws   ║",
        cli.tcp_port + 100
    );
    println!(
        "  ║  Public API   : {}                              ║",
        if cli.public_api {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!(
        "  ║  Public Host  : {}                       ║",
        cli.public_host.as_deref().unwrap_or("auto-detect")
    );
    println!(
        "  ║  Relay Server : {}                              ║",
        if cli.relay_mode {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!(
        "  ║  WebRTC       : /ip4/0.0.0.0/udp/{}/p2p-webrtc-direct ║",
        cli.webrtc_port
    );
    println!(
        "  ║  QUIC         : /ip4/0.0.0.0/udp/{}/quic-v1 ║",
        cli.quic_port
    );
    println!(
        "  ║  Contribute   : {}                              ║",
        if cli.contribute {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!("  ╚══════════════════════════════════════════════╝");
    println!();

    let mut reconnect_tick = tokio::time::interval(Duration::from_secs(cli.reconnect_seconds));
    let mut consensus_tick = tokio::time::interval(Duration::from_secs(1));
    let kad_bootstrap_interval_secs = std::env::var("SHARD_KAD_BOOTSTRAP_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(|v| v.clamp(60, 3600))
        .unwrap_or(300);
    let mut kad_bootstrap_tick =
        tokio::time::interval(Duration::from_secs(kad_bootstrap_interval_secs));
    let mut pending_handshakes: HashMap<OutboundRequestId, PeerId> = HashMap::new();
    let mut pending_layer_queries: HashMap<libp2p::kad::QueryId, (String, u32)> = HashMap::new();
    let mut pending_ledger_sync: HashMap<OutboundRequestId, (PeerId, u64)> = HashMap::new();
    let layer_ttl_ms: u128 = 60_000;
    let bootstrap_url = cli.bootstrap_url.clone();
    let bootstrap_refresh_interval_ms = bootstrap_url_refresh_interval_ms();
    let bootstrap_gossip_interval_ms = bootstrap_gossip_interval_ms();
    let stability_threshold_hours = cli.stability_threshold_hours;
    let mut next_layer_announcement_ms = 0u128;
    let mut next_ledger_snapshot_ms = 0u128;
    let mut next_bootstrap_registry_maintenance_ms = 0u128;
    let mut next_bootstrap_url_refresh_ms = 0u128;
    let mut next_bootstrap_gossip_ms = 0u128;

    // Per-peer exponential backoff for reconnection attempts.
    // Maps peer_id_string -> (consecutive_failures, next_eligible_tick_ms)
    let mut reconnect_backoff: HashMap<String, (u32, u128)> = HashMap::new();

    // ── main event loop ──
    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    tracing::info!("shutdown signal received; entering graceful drain");
                    state.shutdown.store(true, Ordering::Relaxed);
                    let drain_deadline = now_ms() + 30_000;
                    loop {
                        let in_flight = state.in_flight_count.load(Ordering::Relaxed);
                        if in_flight == 0 || now_ms() >= drain_deadline {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }

                    if cli.ha_mode {
                        let leaving = ElectionMessage::NodeLeaving {
                            node_id: state.node_public_key.clone(),
                            peer_id: local_peer_id.to_string(),
                        };
                        if let Ok(payload) = serde_json::to_vec(&leaving) {
                            let _ = swarm
                                .behaviour_mut()
                                .gossipsub
                                .publish(election_topic.clone(), payload);
                        }
                    }

                    let ledger = state.ledger.lock().await;
                    if let Err(error) = state.ledger_store.write_snapshot(&ledger) {
                        tracing::warn!(%error, "failed to flush ledger snapshot during shutdown");
                    }
                    drop(ledger);
                    tracing::info!("Graceful shutdown complete");
                    break;
                }
            }
            _ = consensus_tick.tick() => {
                if let Some(tx) = consensus_in_tx.as_ref() {
                    let peer_count = state.peers.lock().await.len();
                    let _ = tx.send(LeaderInput::PeerCount(peer_count)).await;
                }
            }
            Some(out_msg) = async {
                if let Some(rx) = consensus_out_rx.as_mut() {
                    rx.recv().await
                } else {
                    None
                }
            } => {
                if let Ok(payload) = serde_json::to_vec(&out_msg) {
                    let _ = swarm
                        .behaviour_mut()
                        .gossipsub
                        .publish(election_topic.clone(), payload);
                }
            }
            _ = kad_bootstrap_tick.tick() => {
                let connected = state.peers.lock().await.len();
                if connected > 0 {
                    if let Err(e) = swarm.behaviour_mut().kad.bootstrap() {
                        tracing::debug!(%e, "periodic kademlia bootstrap failed");
                    } else {
                        tracing::debug!("periodic kademlia bootstrap triggered");
                    }
                }
            }
            _ = reconnect_tick.tick() => {
                let mut known = state.known_peers.lock().await.clone();
                let registry_snapshot = state.bootstrap_registry.lock().await.clone();
                let peers_snapshot = state.peers.lock().await.clone();
                let bootstrap_failure_snapshot = state.bootstrap_failures.lock().await.clone();
                known.sort_by(|a, b| {
                    let a_stats = peer_reconnect_stats_for_addr(
                        a,
                        &registry_snapshot,
                        &peers_snapshot,
                        &bootstrap_failure_snapshot,
                    );
                    let b_stats = peer_reconnect_stats_for_addr(
                        b,
                        &registry_snapshot,
                        &peers_snapshot,
                        &bootstrap_failure_snapshot,
                    );
                    a_stats
                        .is_cold
                        .cmp(&b_stats.is_cold)
                        .then(a_stats.bootstrap_failures.cmp(&b_stats.bootstrap_failures))
                        .then(b_stats.stability_score.cmp(&a_stats.stability_score))
                        .then(b_stats.successful_handshakes.cmp(&a_stats.successful_handshakes))
                        .then(a_stats.connection_failures.cmp(&b_stats.connection_failures))
                        .then_with(|| {
                            a_stats
                                .avg_latency_ms
                                .total_cmp(&b_stats.avg_latency_ms)
                        })
                        .then_with(|| reconnect_addr_sort_key(a).cmp(&reconnect_addr_sort_key(b)))
                });
                let connected: HashSet<String> = peers_snapshot.keys().cloned().collect();
                let now = now_ms();
                if let Some(url) = bootstrap_url.as_deref() {
                    if now >= next_bootstrap_url_refresh_ms {
                        match bootstrap_discovery::fetch_bootstrap_peers(url).await {
                            Ok(peers) => {
                                let mut refreshed = 0usize;
                                for peer in peers {
                                    if peer.peer_id.trim().is_empty()
                                        || peer.multiaddr.trim().is_empty()
                                        || peer.peer_id == local_peer_id.to_string()
                                    {
                                        continue;
                                    }
                                    let entry = BootstrapRegistryEntry {
                                        peer_id: peer.peer_id.clone(),
                                        multiaddr: peer.multiaddr.clone(),
                                        stability_score: peer
                                            .stability_score
                                            .unwrap_or(registry_min_score)
                                            .min(100),
                                        uptime_hours: peer.uptime_hours.unwrap_or(0),
                                        version: peer.version.unwrap_or_else(|| "unknown".to_string()),
                                        role: peer.role.clone(),
                                        capability_tier: peer.capability_tier.clone(),
                                        gpu_available: peer.gpu_available,
                                        accepts_scout_work: peer.accepts_scout_work,
                                        public_api: peer.public_api,
                                        public_api_addr: peer.public_api_addr.clone(),
                                        updated_at_ms: now,
                                    };
                                    if upsert_bootstrap_entry(&state, entry).await {
                                        refreshed += 1;
                                    }
                                    if let Ok(addr) = peer.multiaddr.parse::<Multiaddr>() {
                                        let _ = swarm.dial(addr);
                                    }
                                }
                                tracing::info!(
                                    refreshed,
                                    url = %url,
                                    "refreshed bootstrap peers from discovery URL"
                                );
                            }
                            Err(error) => {
                                tracing::warn!(url = %url, %error, "bootstrap discovery refresh failed");
                            }
                        }
                        next_bootstrap_url_refresh_ms = now + bootstrap_refresh_interval_ms;
                    }
                }
                if now >= next_bootstrap_registry_maintenance_ms {
                    let stale_peer_ids = {
                        let mut registry = state.bootstrap_registry.lock().await;
                        let removed =
                            prune_bootstrap_registry(&mut registry, now, registry_ttl_ms);
                        if !removed.is_empty() {
                            save_bootstrap_registry(
                                state.bootstrap_registry_path.as_path(),
                                &registry,
                            )
                            .await;
                        }
                        removed
                    };
                    if !stale_peer_ids.is_empty() {
                        let stale_set: HashSet<String> = stale_peer_ids.into_iter().collect();
                        let removed_addrs = {
                            let mut known = state.known_peers.lock().await;
                            let removed = remove_known_addrs_for_peers(&mut known, &stale_set);
                            if removed > 0 {
                                save_persisted_peers(state.known_peers_path.as_path(), &known).await;
                            }
                            removed
                        };
                        tracing::info!(
                            stale_bootstrap_peers = stale_set.len(),
                            removed_known_addrs = removed_addrs,
                            ttl_ms = registry_ttl_ms,
                            "pruned stale bootstrap registry entries and known-peer seeds"
                        );
                    }
                    next_bootstrap_registry_maintenance_ms = now + (5 * 60 * 1000);
                }
                let max_dials = max_reconnect_dials_per_tick();
                let mut dial_attempts = 0usize;
                let mut attempted_peer_ids: HashSet<String> = HashSet::new();
                for addr_str in known {
                    if let Ok(addr) = addr_str.parse::<Multiaddr>() {
                        if !is_reconnect_candidate_addr(&addr, allow_private_bootstrap) {
                            continue;
                        }
                        if should_attempt_reconnect(&addr, &local_peer_id, &connected) {
                            // Per-peer backoff check
                            let peer_ref = peer_id_from_addr_str(&addr_str)
                                .unwrap_or_else(|| addr_str.clone());
                            if !attempted_peer_ids.insert(peer_ref.clone()) {
                                continue;
                            }
                            if let Some((failures, next_eligible)) = reconnect_backoff.get(&peer_ref) {
                                if now < *next_eligible {
                                    tracing::debug!(
                                        peer_id = %peer_ref,
                                        failures = *failures,
                                        retry_in_ms = (*next_eligible - now),
                                        "reconnect deferred (exponential backoff)"
                                    );
                                    continue;
                                }
                            }
                            if dial_attempts >= max_dials {
                                tracing::debug!(
                                    dial_attempts,
                                    max_dials,
                                    "reconnect dial cap reached for this tick"
                                );
                                break;
                            }

                            if let Err(err) = swarm.dial(addr.clone()) {
                                tracing::debug!(peer_id = %peer_ref, %err, "reconnect dial skipped/failed");
                                // Record failure with backoff: min(20s * 2^failures, 300s)
                                let entry = reconnect_backoff.entry(peer_ref).or_insert((0, 0));
                                entry.0 += 1;
                                let backoff_ms = reconnect_backoff_ms_for_failures(entry.0);
                                entry.1 = now + backoff_ms;
                            } else {
                                tracing::info!(peer_id = %peer_ref, "reconnect dial attempted for disconnected peer");
                            }
                            dial_attempts = dial_attempts.saturating_add(1);
                        } else {
                            // Peer is connected — clear any backoff
                            let peer_ref = peer_id_from_addr_str(&addr_str);
                            if let Some(ref p) = peer_ref {
                                reconnect_backoff.remove(p);
                            }
                        }
                    }
                }

                if now >= next_bootstrap_gossip_ms {
                    let uptime_hours = (now.saturating_sub(state.daemon_start)) / (1000 * 60 * 60);
                    if uptime_hours >= stability_threshold_hours as u128 {
                        let (addrs, public_api_addr) = {
                            let topo = state.topology.lock().await;
                            (
                                topo.listen_addrs.clone(),
                                state.public_host.clone().or(topo.public_api_addr.clone()),
                            )
                        };
                        if let Some(multiaddr) = preferred_bootstrap_multiaddr(&addrs) {
                            let canonical_multiaddr =
                                canonical_bootstrap_multiaddr(&multiaddr, &local_peer_id.to_string());
                            let capability_tier = node_capability_tier(
                                state.node_role.as_str(),
                                state.resource_policy.max_gpu_usage,
                                cli.relay_mode,
                                cli.public_api,
                            );
                            let announcement = BootstrapAnnouncement {
                                peer_id: local_peer_id.to_string(),
                                multiaddr: canonical_multiaddr,
                                stability_score: 100,
                                uptime_hours: uptime_hours as u64,
                                version: env!("CARGO_PKG_VERSION").to_string(),
                                role: Some(state.node_role.clone()),
                                capability_tier: Some(capability_tier.clone()),
                                gpu_available: Some(node_gpu_available_for_tier(capability_tier.as_str())),
                                accepts_scout_work: Some(node_accepts_scout_work(
                                    state.node_role.as_str(),
                                    state.participation_enabled.load(Ordering::Relaxed),
                                    cli.relay_mode,
                                )),
                                public_api: Some(cli.public_api),
                                public_api_addr,
                                announced_at_ms: now,
                            };
                            if let Ok(payload) = serde_json::to_vec(&announcement) {
                                let _ = swarm
                                    .behaviour_mut()
                                    .gossipsub
                                    .publish(bootstrap_topic.clone(), payload);
                            }
                        }
                    }
                    next_bootstrap_gossip_ms = now + bootstrap_gossip_interval_ms;
                }

                // Refresh local layer announcement and query providers for the next layer start.
                let now = now_ms();
                if now >= next_layer_announcement_ms {
                    let announcement = LayerHostAnnouncement {
                        model_id: cli.model_id.clone(),
                        layer_start: cli.layer_start,
                        layer_end: cli.layer_end,
                        peer_id: local_peer_id.to_string(),
                        announced_at_ms: now,
                        expires_at_ms: now + layer_ttl_ms,
                    };
                    if let Ok(payload) = serde_json::to_vec(&announcement) {
                        let _ = swarm
                            .behaviour_mut()
                            .gossipsub
                            .publish(layer_announce_topic.clone(), payload);
                    }
                    {
                        let mut routes = state.layer_routes.lock().await;
                        routes.upsert(announcement);
                        routes.prune_expired(now);
                    }
                    next_layer_announcement_ms = now + (layer_ttl_ms / 2);
                }

                let next_layer_start = cli.layer_end.saturating_add(1);
                let qid = swarm
                    .behaviour_mut()
                    .kad
                    .get_providers(provider_key(&cli.model_id, next_layer_start));
                pending_layer_queries.insert(qid, (cli.model_id.clone(), next_layer_start));

                {
                    let mut router = state.race_router.lock().await;
                    router.prune_expired(now);
                }

                if now >= next_ledger_snapshot_ms {
                    let snapshot_result = {
                        let ledger = state.ledger.lock().await;
                        state.ledger_store.write_snapshot(&ledger)
                    };
                    if let Err(e) = snapshot_result {
                        tracing::warn!(%e, "failed to write ledger snapshot");
                    }
                    next_ledger_snapshot_ms = now + 30_000;
                }

                // Request missing ledger transactions from connected peers.
                let local_head = {
                    let ledger = state.ledger.lock().await;
                    ledger.head().height
                };
                let peers_snapshot = state.peers.lock().await.clone();
                for peer_id_str in peers_snapshot.keys() {
                    if let Ok(peer_id) = peer_id_str.parse::<PeerId>() {
                        let req = LedgerSyncRequest::RangeRequest {
                            from_height: local_head.saturating_add(1),
                            max_items: 512,
                        };
                        let request_id = swarm
                            .behaviour_mut()
                            .ledger_sync
                            .send_request(&peer_id, req);
                        pending_ledger_sync.insert(request_id, (peer_id, local_head));
                    }
                }
            }

            // ── inbound work from Python driver (HTTP → gossipsub) ──
            Some(mut work_req) = work_rx.recv() => {
                if work_req.created_at_ms.is_none() {
                    work_req.created_at_ms = Some(now_ms());
                }

                let nonce = state.credit_nonce.fetch_add(1, Ordering::Relaxed);
                let envelope = shard_common::common::signed_envelope::SignedEnvelope::sign(
                    work_req,
                    &signing_key,
                    nonce,
                    now_ms()
                );

                match serde_json::to_vec(&envelope) {
                    Ok(payload) => {
                        match swarm.behaviour_mut().gossipsub.publish(work_topic.clone(), payload) {
                            Ok(_) => tracing::info!(id = %envelope.payload.request_id, "published Signed WorkRequest to gossipsub"),
                            Err(e) => tracing::warn!(id = %envelope.payload.request_id, %e, "gossipsub publish failed (no peers?)"),
                        }
                    }
                    Err(e) => tracing::error!(%e, "failed to serialize Signed WorkRequest"),
                }
            }

            // ── outbound ban announcements ──
            Some((peer_id, reason)) = ban_rx.recv() => {
                tracing::warn!(%peer_id, %reason, "broadcasting ban to network");
                if let Ok(pid) = peer_id.parse::<libp2p::PeerId>() {
                    let _ = swarm.disconnect_peer_id(pid);
                }
                let announcement = BanAnnouncement {
                    peer_id,
                    reason,
                };
                if let Ok(payload) = serde_json::to_vec(&announcement) {
                    let _ = swarm.behaviour_mut().gossipsub.publish(ban_topic.clone(), payload);
                }
            }

            // ── outbound daemon-scout draft results ──
            Some(draft_response) = draft_publish_rx.recv() => {
                let nonce = state.credit_nonce.fetch_add(1, Ordering::Relaxed);
                let envelope = shard_common::common::signed_envelope::SignedEnvelope::sign(
                    draft_response.clone(),
                    &signing_key,
                    nonce,
                    now_ms(),
                );
                match serde_json::to_vec(&envelope) {
                    Ok(payload) => {
                        match swarm.behaviour_mut().gossipsub.publish(result_topic.clone(), payload) {
                            Ok(_) => tracing::info!(
                                request_id = %draft_response.request_id,
                                tokens = draft_response.draft_tokens.len(),
                                "published daemon-scout WorkResponse to gossipsub"
                            ),
                            Err(e) => tracing::warn!(
                                request_id = %draft_response.request_id,
                                %e,
                                "daemon-scout gossipsub publish failed"
                            ),
                        }
                    }
                    Err(e) => tracing::error!(%e, "failed to serialize daemon-scout WorkResponse"),
                }
            }

            // ── inbound pipeline forward requests (HTTP -> pooled gossipsub fanout) ──
            Some(dispatch) = pipeline_rx.recv() => {
                let pool_size = state.race_pool_size.clamp(1, 8);
                let pool = {
                    let mut routes = state.layer_routes.lock().await;
                    routes.prune_expired(now_ms());
                    routes.find_next_layer_peers(&dispatch.model_id, dispatch.current_layer, pool_size)
                };
                if pool.is_empty() {
                    tracing::warn!(
                        request_id = %dispatch.packet.request_id,
                        step_id = %dispatch.packet.step_id,
                        model_id = %dispatch.model_id,
                        current_layer = dispatch.current_layer,
                        "pipeline dispatch skipped; no next-layer peers found"
                    );
                    continue;
                }

                let race_key = RaceKey {
                    request_id: dispatch.packet.request_id.clone(),
                    step_id: dispatch.packet.step_id.clone(),
                };

                // Phase B: Long-context guard bypass check
                let is_long_context = {
                    let reqs = state.active_requests.lock().await;
                    reqs.get(&dispatch.packet.request_id)
                        .map(|req| req.input_token_count > state.fallback_config.long_context_threshold)
                        .unwrap_or(false)
                };

                if is_long_context {
                    tracing::info!(
                        request_id = %dispatch.packet.request_id,
                        step_id = %dispatch.packet.step_id,
                        "long context RAG prompt detected; bypassing peer mesh and triggering centralized fallback",
                    );

                    let state_clone = state.clone();
                    let packet_clone = dispatch.packet.clone();
                    tokio::spawn(async move {
                        let request_state = {
                            let reqs = state_clone.active_requests.lock().await;
                            reqs.get(&packet_clone.request_id).cloned()
                        };

                        if let Some(req) = request_state {
                            state_clone.system_metrics.inc_fallback_invocations();
                            match execute_centralized_fallback(&state_clone.fallback_config, &req).await {
                                Ok(response_text) => {
                                    tracing::info!(request_id = %packet_clone.request_id, "Centralized fallback returned response");
                                    let mut results = state_clone.results.lock().await;
                                    results.push_back(WorkResponse {
                                        request_id: packet_clone.request_id.clone(),
                                        peer_id: "centralized-fallback".to_string(),
                                        draft_tokens: Vec::new(),
                                        draft_text: response_text,
                                        latency_ms: 0.0,
                                        created_at_ms: Some(now_ms()),
                                    });
                                }
                                Err(e) => {
                                    tracing::error!(request_id = %packet_clone.request_id, "Centralized fallback failed: {}", e);
                                    let mut results = state_clone.results.lock().await;
                                    results.push_back(WorkResponse {
                                        request_id: packet_clone.request_id.clone(),
                                        peer_id: "centralized-fallback".to_string(),
                                        draft_tokens: Vec::new(),
                                        draft_text: "Fallback inference failed".to_string(),
                                        latency_ms: 0.0,
                                        created_at_ms: Some(now_ms()),
                                    });
                                }
                            }
                        }
                    });

                    continue; // Bypass starting race and broadcasting forward pass via mesh
                }

                {
                    let mut router = state.race_router.lock().await;
                    router.start_race(
                        race_key,
                        dispatch.packet.shape.clone(),
                        format!("{:?}", dispatch.packet.format).to_lowercase(),
                        pool.clone(),
                        now_ms().saturating_add(state.race_timeout_ms as u128),
                    );
                }

                for peer in &pool {
                    let mut p = dispatch.packet.clone();
                    p.target_peer_id = Some(peer.clone());
                    p.target_peer_pool = Some(pool.clone());
                    if p.created_at_ms.is_none() {
                        p.created_at_ms = Some(now_ms());
                    }
                    if let Ok(payload) = serde_json::to_vec(&TrainingGossipPacket::ForwardPass(p)) {
                        let _ = swarm.behaviour_mut().gossipsub.publish(forward_topic.clone(), payload);
                    }
                }
            }

            // ── inbound browser layer results (HTTP -> gossipsub forward-result) ──
            Some(packet) = browser_result_rx.recv() => {
                if let Ok(payload) = serde_json::to_vec(&TrainingGossipPacket::ForwardPass(packet)) {
                    let _ = swarm
                        .behaviour_mut()
                        .gossipsub
                        .publish(forward_result_topic.clone(), payload);
                }
            }

            // ── swarm events ──
            event = swarm.select_next_some() => {
                match event {
                    // ── gossipsub ──
                    SwarmEvent::Behaviour(ShardBehaviourEvent::Gossipsub(gossipsub::Event::Message { message, .. })) => {
                        if message.topic == result_topic.hash() {
                            if let Ok(result) = shard_verifier::verification::draft_verifier::verify_draft_submission(&message.data, state.envelope_verifier.clone()).await {
                                let peer_is_blackholed = {
                                    let mut penalties = state.scout_penalties.lock().await;
                                    penalties.is_blackholed(&result.peer_id)
                                };
                                if peer_is_blackholed {
                                    tracing::warn!(peer = %result.peer_id, "dropping WorkResponse from blackholed scout peer");
                                    if let Ok(pid) = result.peer_id.parse::<libp2p::PeerId>() {
                                        let _ = swarm.disconnect_peer_id(pid);
                                    }
                                    continue;
                                }

                                tracing::info!(
                                    request_id = %result.request_id,
                                    peer = %result.peer_id,
                                    tokens = result.draft_tokens.len(),
                                    "received WorkResponse via gossipsub"
                                );

                                // Propagation latency telemetry is intentionally lightweight:
                                // one saturating subtraction + one atomic increment.
                                if let Some(created_at_ms) = result.created_at_ms {
                                    let propagation_ms = now_ms().saturating_sub(created_at_ms) as u64;
                                    state.gossipsub_latency_hist.observe(propagation_ms);
                                }

                                // Insert into idempotent_results so wait_for_scout_draft can find it
                                {
                                    let mut by_id = state.idempotent_results.lock().await;
                                    by_id.insert(result.request_id.clone(), result.clone());
                                }

                                // Insert into scout_draft_mailbox for deterministic handoff
                                {
                                    let draft = ScoutDraft {
                                        work_id: result.request_id.clone(),
                                        scout_id: result.peer_id.clone(),
                                        draft_tokens: result.draft_tokens.clone(),
                                        draft_text: result.draft_text.clone(),
                                        timestamp_ms: result.created_at_ms.unwrap_or_else(now_ms),
                                        latency_ms: result.latency_ms as u64,
                                    };
                                    push_scout_draft(&state, draft).await;
                                }

                                let mut q = state.results.lock().await;
                                q.push_back(result);
                                while q.len() > 128 { q.pop_front(); }
                            }
                        } else if message.topic == work_topic.hash() {
                            match serde_json::from_slice::<shard_common::common::signed_envelope::SignedEnvelope<WorkRequest>>(&message.data) {
                                Ok(envelope) => {
                                    let mut verifier = state.envelope_verifier.lock().await;
                                    if let Err(e) = verifier.verify_full(&envelope) {
                                        tracing::warn!(%e, "received invalid Signed WorkRequest; dropping");
                                        continue;
                                    }

                                    tracing::info!(id = %envelope.payload.request_id, signer = %envelope.signer_pubkey_hex, "received valid Signed WorkRequest via gossipsub");

                                    let work = envelope.payload;

                                    // Browser/API scouts attached to this verifier can only submit
                                    // drafts back here, so only expose work we locally own.
                                    let locally_pending = {
                                        let pending = state.speculative_pending.lock().await;
                                        pending.contains_key(work.request_id.as_str())
                                    };
                                    if locally_pending {
                                        let mut queue = state.scout_work.lock().await;
                                        enqueue_scout_work(&mut queue, work.clone());
                                    } else {
                                        record_speculative_trace(
                                            &state,
                                            work.request_id.clone(),
                                            "gossip_work_skip_browser_queue",
                                            None,
                                            Some(
                                                "received foreign speculative work; not exposing it to local browser scouts"
                                                    .to_string(),
                                            ),
                                            None,
                                        )
                                        .await;
                                    }

                                    // ── Daemon-side scout worker ──
                                    // If this node contributes compute and has a local engine,
                                    // generate draft tokens and publish them back via gossipsub.
                                    let contribute_enabled = {
                                        let topo = state.topology.lock().await;
                                        topo.contribute_enabled
                                    };
                                    if contribute_enabled {
                                        let scout_state = state.clone();
                                        let scout_peer_id = local_peer_id.to_string();
                                        tokio::spawn(async move {
                                            let draft_start = now_ms();
                                            let mut engine_guard = scout_state.engine.lock().await;
                                            let engine = match engine_guard.as_mut() {
                                                Some(e) => e,
                                                None => {
                                                    tracing::debug!(
                                                        request_id = %work.request_id,
                                                        "daemon scout: no local engine available"
                                                    );
                                                    return;
                                                }
                                            };

                                            let mut tokens = match engine.tokenize(&work.prompt_context, 4096) {
                                                Ok(t) => t,
                                                Err(_) => return,
                                            };
                                            if !tokens.is_empty() && tokens[0] == 128000 {
                                                tokens.remove(0);
                                            }
                                            if engine.eval(&tokens).is_err() {
                                                return;
                                            }

                                            let target = (work.min_tokens.max(4) as usize).min(32);
                                            let mut draft_tokens = Vec::with_capacity(target);
                                            let mut draft_text = String::new();

                                            for _ in 0..target {
                                                let logits = match engine.get_logits(128256) {
                                                    Ok(l) => l,
                                                    Err(_) => break,
                                                };
                                                let mut best_idx = 0usize;
                                                let mut best_val = -f32::INFINITY;
                                                for (i, &val) in logits.iter().enumerate() {
                                                    if val > best_val {
                                                        best_val = val;
                                                        best_idx = i;
                                                    }
                                                }
                                                // Stop on EOS / EOT tokens
                                                if best_idx == 128001 || best_idx == 128009 {
                                                    break;
                                                }
                                                draft_tokens.push(best_idx as i32);
                                                if let Ok(piece) = engine.token_to_piece(best_idx as i32) {
                                                    draft_text.push_str(&piece);
                                                }
                                                if engine.eval(&[best_idx as i32]).is_err() {
                                                    break;
                                                }
                                            }

                                            drop(engine_guard);

                                            if draft_tokens.is_empty() {
                                                return;
                                            }

                                            let latency_ms = (now_ms() - draft_start) as f32;
                                            let response = WorkResponse {
                                                request_id: work.request_id.clone(),
                                                peer_id: scout_peer_id,
                                                draft_tokens,
                                                draft_text,
                                                latency_ms,
                                                created_at_ms: Some(now_ms()),
                                            };

                                            tracing::info!(
                                                request_id = %work.request_id,
                                                tokens = response.draft_tokens.len(),
                                                latency_ms = %format!("{:.0}", latency_ms),
                                                "daemon scout: generated draft tokens"
                                            );

                                            if let Err(e) = scout_state.draft_publish_tx.send(response).await {
                                                tracing::warn!(%e, "daemon scout: failed to send draft to publish channel");
                                            }
                                        });
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(%e, "received unsigned or malformed WorkRequest; dropping");
                                }
                            }
                        } else if message.topic == ledger_topic.hash() {
                            match serde_json::from_slice::<ComputeCreditTx>(&message.data) {
                                Ok(tx) => {
                                    let mut ledger = state.ledger.lock().await;
                                    if let Err(e) = ledger.apply_signed_tx(tx.clone()) {
                                        tracing::warn!(%e, "failed to apply credit transaction");
                                    } else if let Err(e) =
                                        state.ledger_store.append_tx(&tx, ledger.head())
                                    {
                                        tracing::warn!(%e, "failed to persist applied credit transaction");
                                    }
                                }
                                Err(e) => tracing::warn!(%e, "invalid ledger transaction packet; ignoring"),
                            }
                        } else if message.topic == layer_announce_topic.hash() {
                            match serde_json::from_slice::<LayerHostAnnouncement>(&message.data) {
                                Ok(ann) => {
                                    let mut routes = state.layer_routes.lock().await;
                                    routes.upsert(ann);
                                    routes.prune_expired(now_ms());
                                }
                                Err(e) => tracing::warn!(%e, "invalid layer announcement packet; ignoring"),
                            }
                        } else if message.topic == bootstrap_topic.hash() {
                            match serde_json::from_slice::<BootstrapAnnouncement>(&message.data) {
                                Ok(ann) => {
                                    if ann.peer_id == local_peer_id.to_string() {
                                        continue;
                                    }
                                    let entry = BootstrapRegistryEntry {
                                        peer_id: ann.peer_id.clone(),
                                        multiaddr: ann.multiaddr.clone(),
                                        stability_score: ann.stability_score.min(100),
                                        uptime_hours: ann.uptime_hours,
                                        version: ann.version.clone(),
                                        role: ann.role.clone(),
                                        capability_tier: ann.capability_tier.clone(),
                                        gpu_available: ann.gpu_available,
                                        accepts_scout_work: ann.accepts_scout_work,
                                        public_api: ann.public_api,
                                        public_api_addr: ann.public_api_addr.clone(),
                                        updated_at_ms: ann.announced_at_ms.max(now_ms()),
                                    };
                                    let changed = upsert_bootstrap_entry(&state, entry).await;
                                    if changed {
                                        tracing::info!(
                                            peer_id = %ann.peer_id,
                                            multiaddr = %ann.multiaddr,
                                            "learned bootstrap peer via gossipsub"
                                        );
                                    }
                                    if let Ok(addr) = ann.multiaddr.parse::<Multiaddr>() {
                                        if should_attempt_reconnect(&addr, &local_peer_id, &state.peers.lock().await.keys().cloned().collect()) {
                                            let _ = swarm.dial(addr);
                                        }
                                    }
                                }
                                Err(e) => tracing::warn!(%e, "invalid bootstrap announcement packet; ignoring"),
                            }
                        } else if message.topic == ban_topic.hash() {
                            #[derive(Deserialize)]
                            struct BanAnnouncement {
                                peer_id: String,
                                reason: String,
                            }
                            if let Ok(ban) = serde_json::from_slice::<BanAnnouncement>(&message.data) {
                                tracing::info!(peer = %ban.peer_id, reason = %ban.reason, "received ban announcement via gossipsub");
                                // We trust other verifiers for now (Phase 3 simplified)
                                if let Ok(pid) = ban.peer_id.parse::<libp2p::PeerId>() {
                                    let _ = swarm.disconnect_peer_id(pid);
                                    // Also add to local blacklist
                                    let mut penalties = state.scout_penalties.lock().await;
                                    penalties.apply_update(ScoutPenaltyUpdate {
                                        peer_id: ban.peer_id.clone(),
                                        accepted: false,
                                        probability_bound: 0.0,
                                        latency_ms: None,
                                        reason: Some(format!("Network ban: {}", ban.reason)),
                                    }, 0);
                                }
                            }
                        } else if cli.ha_mode && message.topic == election_topic.hash() {
                            if let Ok(msg) = serde_json::from_slice::<ElectionMessage>(&message.data) {
                                if let Some(tx) = consensus_in_tx.as_ref() {
                                    let from_peer = message.source.map(|peer| peer.to_string());
                                    let _ = tx
                                        .send(LeaderInput::NetworkMessage {
                                            from_peer,
                                            message: msg,
                                        })
                                        .await;
                                }
                            }
                        } else if message.topic == forward_result_topic.hash() {
                            if let Ok(TrainingGossipPacket::ForwardPass(packet)) =
                                serde_json::from_slice::<TrainingGossipPacket>(&message.data)
                            {
                                let is_for_me = packet
                                    .target_peer_id
                                    .as_deref()
                                    .map(|p| p == local_peer_id.to_string())
                                    .unwrap_or(false);
                                if !is_for_me {
                                    continue;
                                }
                                let key = RaceKey {
                                    request_id: packet.request_id.clone(),
                                    step_id: packet.step_id.clone(),
                                };
                                let dtype = format!("{:?}", packet.format).to_lowercase();
                                let outcome = {
                                    let mut router = state.race_router.lock().await;
                                    router.submit_candidate(
                                        now_ms(),
                                        &key,
                                        &packet.source_peer_id,
                                        &packet.shape,
                                        &dtype,
                                    )
                                };
                                match outcome {
                                    RaceSubmitOutcome::AcceptedFirst => {
                                        tracing::info!(
                                            request_id = %packet.request_id,
                                            step_id = %packet.step_id,
                                            winner = %packet.source_peer_id,
                                            "accepted first forward tensor from race pool"
                                        );

                                        let nonce = state.credit_nonce.fetch_add(1, Ordering::Relaxed);
                                        let tx = LedgerState::sign_reward_tx(
                                            &signing_key,
                                            &node_wallet,
                                            &packet.source_peer_id,
                                            1,
                                            &packet.request_id,
                                            &packet.step_id,
                                            nonce,
                                            now_ms(),
                                        );
                                        {
                                            let mut ledger = state.ledger.lock().await;
                                            if let Err(e) = ledger.apply_signed_tx(tx.clone()) {
                                                tracing::warn!(%e, "failed to apply locally generated credit transaction");
                                            } else if let Err(e) =
                                                state.ledger_store.append_tx(&tx, ledger.head())
                                            {
                                                tracing::warn!(%e, "failed to persist locally generated credit transaction");
                                            }
                                        }
                                        if let Ok(payload) = serde_json::to_vec(&tx) {
                                            let _ = swarm
                                                .behaviour_mut()
                                                .gossipsub
                                                .publish(ledger_topic.clone(), payload);
                                        }
                                    }
                                    RaceSubmitOutcome::RejectedLate => {
                                        tracing::debug!(
                                            request_id = %packet.request_id,
                                            step_id = %packet.step_id,
                                            peer = %packet.source_peer_id,
                                            "dropped late forward tensor response"
                                        );
                                    }
                                    RaceSubmitOutcome::RejectedInvalid => {
                                        tracing::warn!(
                                            request_id = %packet.request_id,
                                            step_id = %packet.step_id,
                                            peer = %packet.source_peer_id,
                                            "rejected invalid forward tensor response"
                                        );
                                    }
                                    RaceSubmitOutcome::UnknownRace => {}
                                    RaceSubmitOutcome::TimedOut => {
                                        tracing::warn!(
                                            request_id = %packet.request_id,
                                            step_id = %packet.step_id,
                                            "race timed out — triggering fallback"
                                        );
                                    }
                                }
                            }
                        } else if message.topic == forward_topic.hash() || message.topic == backward_topic.hash() {
                            match serde_json::from_slice::<TrainingGossipPacket>(&message.data) {
                                Ok(TrainingGossipPacket::ForwardPass(packet)) => {
                                    if let Some(chunk) = packet.chunk.as_ref() {
                                        if let Some(wire_hex) = chunk.data.strip_prefix("wire1:") {
                                            if let Ok(raw) = hex::decode(wire_hex) {
                                                let _ = TensorWirePacket::decode(&raw);
                                            }
                                        }
                                    }
                                    tracing::info!(
                                        request_id = %packet.request_id,
                                        step_id = %packet.step_id,
                                        tensor = %packet.tensor_name,
                                        source_peer = %packet.source_peer_id,
                                        target_peer = ?packet.target_peer_id,
                                        has_chunk = packet.chunk.is_some(),
                                        has_blob_ref = packet.blob_ref.is_some(),
                                        "received forward-pass activation packet"
                                    );

                                    // If this node is one of the selected pool peers, compute and return tensor.
                                    let is_targeted_to_me = packet
                                        .target_peer_id
                                        .as_deref()
                                        .map(|p| p == local_peer_id.to_string())
                                        .unwrap_or(false);
                                    if is_targeted_to_me {
                                        let browser_session = {
                                            let mut sessions = state.browser_sessions.lock().await;
                                            prune_browser_sessions(&mut sessions, now_ms());
                                            sessions
                                                .values()
                                                .find(|session| session.model_id == cli.model_id)
                                                .map(|session| {
                                                    (
                                                        session.session_id.clone(),
                                                        session.obfuscation_key.clone(),
                                                    )
                                                })
                                        };

                                        if let Some((session_id, obfuscation_key)) = browser_session {
                                            if let Some(chunk) = packet.chunk.as_ref() {
                                                if let Some(wire_hex) = chunk.data.strip_prefix("wire1:") {
                                                    if let Ok(raw) = hex::decode(wire_hex) {
                                                        if let Ok(wire) = TensorWirePacket::decode(&raw) {
                                                            let nonce = random_nonce();
                                                            let obfuscated =
                                                                obfuscate_bytes(&obfuscation_key, &nonce, &wire.data);
                                                            let work_item = BrowserLayerWorkItem {
                                                                work_id: uuid::Uuid::new_v4().to_string(),
                                                                session_id,
                                                                request_id: packet.request_id.clone(),
                                                                step_id: packet.step_id.clone(),
                                                                source_peer_id: packet.source_peer_id.clone(),
                                                                tensor_name: wire.tensor_name,
                                                                dtype: wire.dtype,
                                                                shape: wire.shape,
                                                                nonce_hex: hex::encode(nonce),
                                                                obfuscated_tensor_hex: hex::encode(obfuscated),
                                                                created_at_ms: now_ms(),
                                                            };
                                                            let mut queue = state.browser_work.lock().await;
                                                            queue.push_back(work_item);
                                                            while queue.len() > 2048 {
                                                                queue.pop_front();
                                                            }
                                                            continue;
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        let mut result = packet.clone();
                                        result.source_peer_id = local_peer_id.to_string();
                                        result.target_peer_id = Some(packet.source_peer_id.clone());
                                        result.created_at_ms = Some(now_ms());
                                        if let Ok(payload) =
                                            serde_json::to_vec(&TrainingGossipPacket::ForwardPass(result))
                                        {
                                            let _ = swarm
                                                .behaviour_mut()
                                                .gossipsub
                                                .publish(forward_result_topic.clone(), payload);
                                        }
                                    }
                                }
                                Ok(TrainingGossipPacket::BackwardPass(packet)) => {
                                    tracing::info!(
                                        request_id = %packet.request_id,
                                        step_id = %packet.step_id,
                                        microbatch_id = %packet.microbatch_id,
                                        layer = %packet.layer_path,
                                        tensor = %packet.tensor_name,
                                        source_peer = %packet.source_peer_id,
                                        target_peer = ?packet.target_peer_id,
                                        has_chunk = packet.chunk.is_some(),
                                        has_blob_ref = packet.blob_ref.is_some(),
                                        "received backward-pass gradient packet"
                                    );

                                    // Scaffold only: retain the latest gradient packets until
                                    // training routing logic is implemented.
                                    let mut gradients = state.backward_passes.lock().await;
                                    gradients.push_back(packet);
                                    while gradients.len() > 128 { gradients.pop_front(); }
                                }
                                Err(e) => {
                                    tracing::warn!(%e, "invalid training gossip packet; ignoring");
                                }
                            }
                        }
                    }

                    // ── request/response: work forwarding ──
                    SwarmEvent::Behaviour(ShardBehaviourEvent::ControlWork(
                        request_response::Event::Message {
                            message:
                                request_response::Message::Request {
                                    request, channel, ..
                                },
                            ..
                        },
                    )) => {
                        tracing::info!(
                            id = %request.request_id,
                            "work request via req/resp -> publishing signed envelope to gossipsub"
                        );

                        let nonce = state.credit_nonce.fetch_add(1, Ordering::Relaxed);
                        let envelope = shard_common::common::signed_envelope::SignedEnvelope::sign(
                            request,
                            &signing_key,
                            nonce,
                            now_ms()
                        );

                        if let Ok(payload) = serde_json::to_vec(&envelope) {
                            let _ = swarm
                                .behaviour_mut()
                                .gossipsub
                                .publish(work_topic.clone(), payload);
                        }
                        let _ = swarm.behaviour_mut().control_work.send_response(
                            channel,
                            "published shard-work".to_string(),
                        );
                    }

                    // ── handshake (PING/PONG) ──
                    SwarmEvent::Behaviour(ShardBehaviourEvent::Handshake(
                        request_response::Event::Message { peer, message, .. },
                    )) => {
                        match message {
                            request_response::Message::Request { request, channel, .. } => {
                                if request.kind == "PING" {
                                    let latency = now_ms().saturating_sub(request.sent_at_ms);
                                    tracing::info!(%peer, %latency, "PING → PONG");
                                    let pong = Heartbeat { kind: "PONG".into(), sent_at_ms: now_ms() };
                                    let _ = swarm.behaviour_mut().handshake.send_response(channel, pong);

                                    let mut peers = state.peers.lock().await;
                                    if let Some(info) = peers.get_mut(&peer.to_string()) {
                                        info.verified = true;
                                        info.last_seen_at = now_ms();
                                        info.successful_handshakes =
                                            info.successful_handshakes.saturating_add(1);
                                        info.connection_failures = 0;
                                        let latency_sample = latency as f32;
                                        info.avg_latency_ms = if info.avg_latency_ms <= 0.0 {
                                            latency_sample
                                        } else {
                                            (info.avg_latency_ms * 0.7) + (latency_sample * 0.3)
                                        };
                                    }
                                }
                            }
                            request_response::Message::Response { response, request_id } => {
                                tracing::info!(%peer, kind = %response.kind, "handshake response");
                                pending_handshakes.remove(&request_id);
                                let mut peers = state.peers.lock().await;
                                if let Some(info) = peers.get_mut(&peer.to_string()) {
                                    info.last_seen_at = now_ms();
                                    if response.kind == "PONG" {
                                        info.verified = true;
                                        info.successful_handshakes =
                                            info.successful_handshakes.saturating_add(1);
                                        info.connection_failures = 0;
                                    }
                                }
                            }
                        }
                    }

                    // ── ledger sync protocol ──
                    SwarmEvent::Behaviour(ShardBehaviourEvent::LedgerSync(event)) => {
                        match event {
                            request_response::Event::Message { peer, message, .. } => match message {
                                request_response::Message::Request {
                                    request, channel, ..
                                } => {
                                    let response = match request {
                                        LedgerSyncRequest::HeadRequest => {
                                            let ledger = state.ledger.lock().await;
                                            LedgerSyncResponse::HeadResponse { head: ledger.head() }
                                        }
                                        LedgerSyncRequest::RangeRequest {
                                            from_height,
                                            max_items,
                                        } => {
                                            let ledger = state.ledger.lock().await;
                                            let export =
                                                ledger.export_range(from_height, max_items.clamp(1, 4096));
                                            LedgerSyncResponse::RangeResponse {
                                                from_height: export.from_height,
                                                end_height: export.end_height,
                                                has_more: export.has_more,
                                                txs: export.txs,
                                            }
                                        }
                                        LedgerSyncRequest::HashProbe {
                                            from_height,
                                            to_height,
                                        } => {
                                            let ledger = state.ledger.lock().await;
                                            let hashes = hash_probe_segments(
                                                ledger.txs(),
                                                from_height,
                                                to_height,
                                                64,
                                            );
                                            LedgerSyncResponse::HashProbeResponse {
                                                from_height,
                                                to_height,
                                                segment_hashes: hashes,
                                            }
                                        }
                                    };
                                    let _ = swarm
                                        .behaviour_mut()
                                        .ledger_sync
                                        .send_response(channel, response);
                                }
                                request_response::Message::Response {
                                    response,
                                    request_id,
                                } => {
                                    let _ = pending_ledger_sync.remove(&request_id);
                                    if let LedgerSyncResponse::RangeResponse { txs, .. } = response {
                                        if !txs.is_empty() {
                                            let mut ledger = state.ledger.lock().await;
                                            for tx in txs {
                                                if ledger.apply_signed_tx(tx.clone()).is_ok() {
                                                    if let Err(e) =
                                                        state.ledger_store.append_tx(&tx, ledger.head())
                                                    {
                                                        tracing::warn!(
                                                            %e,
                                                            %peer,
                                                            "failed to persist synced ledger tx"
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                            request_response::Event::OutboundFailure {
                                request_id,
                                error,
                                peer,
                                ..
                            } => {
                                pending_ledger_sync.remove(&request_id);
                                tracing::debug!(%peer, %error, "ledger sync outbound failure");
                            }
                            request_response::Event::InboundFailure { peer, error, .. } => {
                                tracing::debug!(%peer, %error, "ledger sync inbound failure");
                            }
                            request_response::Event::ResponseSent { peer, .. } => {
                                tracing::debug!(%peer, "ledger sync response sent");
                            }
                        }
                    }

                    // ── verify protocol ──
                    SwarmEvent::Behaviour(ShardBehaviourEvent::Verify(event)) => {
                        match event {
                            request_response::Event::Message { peer, message, .. } => match message {
                                request_response::Message::Request {
                                    request, channel, ..
                                } => {
                                    tracing::debug!(%peer, "received draft submission");
                                    if let Some(draft) =
                                        process_draft_submission(&state, request).await
                                    {
                                        push_scout_draft(&state, draft).await;
                                    }
                                    let _ = swarm
                                        .behaviour_mut()
                                        .verify
                                        .send_response(channel, "ok".to_string());
                                }
                                request_response::Message::Response { response, .. } => {
                                    tracing::debug!(%peer, %response, "draft submission response");
                                }
                            },
                            other => tracing::debug!(?other, "verify protocol event"),
                        }
                    }

                    // ── kademlia ──
                    SwarmEvent::Behaviour(ShardBehaviourEvent::Kad(event)) => {
                        match event {
                            KadEvent::OutboundQueryProgressed { id, result, .. } => {
                                match result {
                                    QueryResult::GetProviders(Ok(GetProvidersOk::FoundProviders { providers, .. })) => {
                                        if let Some((model_id, layer_start)) = pending_layer_queries.get(&id).cloned() {
                                            let now = now_ms();
                                            let expires = now + layer_ttl_ms;
                                            let mut routes = state.layer_routes.lock().await;
                                            for peer in providers {
                                                let ann = LayerHostAnnouncement {
                                                    model_id: model_id.clone(),
                                                    layer_start,
                                                    layer_end: layer_start,
                                                    peer_id: peer.to_string(),
                                                    announced_at_ms: now,
                                                    expires_at_ms: expires,
                                                };
                                                routes.upsert(ann);
                                            }
                                            routes.prune_expired(now);
                                        }
                                    }
                                    QueryResult::GetProviders(Ok(GetProvidersOk::FinishedWithNoAdditionalRecord { .. })) => {
                                        pending_layer_queries.remove(&id);
                                    }
                                    QueryResult::StartProviding(Ok(_)) => {
                                        tracing::debug!("kademlia start_providing succeeded");
                                    }
                                    other => {
                                        tracing::debug!(?other, "kademlia query progressed");
                                    }
                                }
                            }
                            other => tracing::debug!(?other, "kademlia event"),
                        }
                    }

                    SwarmEvent::Behaviour(ShardBehaviourEvent::RelayClient(event)) => {
                        tracing::info!("RelayClient event: {:?}", event);
                        match event {
                            relay::client::Event::ReservationReqAccepted { .. } => {
                                let mut topo = state.topology.lock().await;
                                topo.relay_reservation_active = true;
                            }
                            relay::client::Event::OutboundCircuitEstablished { .. } => {
                                let mut topo = state.topology.lock().await;
                                topo.relay_reservation_active = true;
                            }
                            _ => {}
                        }
                    }

                    // ── relay server ──
                    SwarmEvent::Behaviour(ShardBehaviourEvent::RelayServer(event)) => {
                        match event {
                            relay::Event::ReservationReqAccepted { src_peer_id, .. } => {
                                tracing::info!(%src_peer_id, "relay server: reservation accepted");
                            }
                            relay::Event::ReservationReqDenied { src_peer_id, .. } => {
                                tracing::warn!(%src_peer_id, "relay server: reservation denied");
                            }
                            _ => {}
                        }
                    }

                    // ── dcutr ──
                    SwarmEvent::Behaviour(ShardBehaviourEvent::Dcutr(event)) => {
                        let _ = event;
                        // dcutr events - simplified for compatibility
                        tracing::debug!("dcutr event: {:?}", event);
                    }

                    // ── autonat ──
                    SwarmEvent::Behaviour(ShardBehaviourEvent::Autonat(
                        autonat::Event::StatusChanged { old, new },
                    )) => {
                        tracing::info!(?old, ?new, "AutoNAT status changed");
                        let mut topo = state.topology.lock().await;
                        match new {
                            autonat::NatStatus::Public(_) => {
                                topo.is_public = true;
                                topo.nat_status = "public".to_string();
                            }
                            autonat::NatStatus::Private => {
                                topo.is_public = false;
                                topo.nat_status = "private".to_string();
                            }
                            _ => {}
                        }
                    }

                    // ── identify ──
                    SwarmEvent::Behaviour(ShardBehaviourEvent::Identify(event)) => {
                        match event {
                            identify::Event::Received { peer_id, info, .. } => {
                                tracing::info!(%peer_id, protocol_version = %info.protocol_version, "identify info received");
                                let observed_addr = info.observed_addr;
                                tracing::debug!(%peer_id, "observed address received");
                                let mut topo = state.topology.lock().await;
                                let observed_is_loopback = observed_addr.to_string().starts_with("/ip4/127.")
                                    || observed_addr.to_string().starts_with("/ip6/::1");
                                let observed_is_private = is_non_public_bootstrap_addr(&observed_addr);
                                if !observed_is_loopback
                                    && (!observed_is_private || allow_private_bootstrap)
                                {
                                    swarm.add_external_address(observed_addr.clone());
                                    if topo.public_api_addr.is_none() {
                                        topo.public_api_addr =
                                            Some(format!("{}/p2p/{}", observed_addr, local_peer_id));
                                    }
                                }

                                let mut learned_addrs = Vec::new();
                                for listen_addr in &info.listen_addrs {
                                    let mut full_addr = listen_addr.clone();
                                    if !full_addr.iter().any(|p| matches!(p, libp2p::multiaddr::Protocol::P2p(_))) {
                                        full_addr = full_addr.with(libp2p::multiaddr::Protocol::P2p(peer_id));
                                    }
                                    if !is_reconnect_candidate_addr(
                                        &full_addr,
                                        allow_private_bootstrap,
                                    ) {
                                        tracing::debug!(
                                            %peer_id,
                                            addr = %full_addr,
                                            "skipping non-dialable identify listen address"
                                        );
                                        continue;
                                    }
                                    swarm.behaviour_mut().kad.add_address(&peer_id, full_addr.clone());
                                    learned_addrs.push(full_addr.to_string());
                                }
                                if !learned_addrs.is_empty() {
                                    let mut known = state.known_peers.lock().await;
                                    known.extend(learned_addrs);
                                    *known = unique_addrs(known.clone());
                                    save_persisted_peers(&known_peers_path, &known).await;
                                }

                                if !cli.relay_mode && cli.nat_traversal {
                                    // Check if the peer supports being a relay server
                                    let is_relay = info.protocols.iter().any(|p| p.as_ref() == "/libp2p/circuit/relay/0.2.0/hop");
                                    if is_relay {
                                        let has_relay_listen = swarm.listeners().any(|a| {
                                            a.iter().any(|p| matches!(p, libp2p::multiaddr::Protocol::P2pCircuit))
                                        });
                                        if !has_relay_listen {
                                            // Try to find a public address for the relay
                                            if let Some(relay_addr) = info.listen_addrs.iter().find(|a| !a.to_string().contains("127.0.0.1") && !a.to_string().contains("::1")) {
                                                // Make sure the address contains the peer_id
                                                let mut full_relay_addr = relay_addr.clone();
                                                if !full_relay_addr.iter().any(|p| matches!(p, libp2p::multiaddr::Protocol::P2p(_))) {
                                                    full_relay_addr = full_relay_addr.with(libp2p::multiaddr::Protocol::P2p(peer_id));
                                                }
                                                let p2p_circuit_addr = full_relay_addr.with(libp2p::multiaddr::Protocol::P2pCircuit);

                                                tracing::info!(%peer_id, "found relay server; attempting reservation");
                                                let _ = swarm.listen_on(p2p_circuit_addr);
                                            }
                                        }
                                    }
                                }
                            }
                            identify::Event::Sent { .. } => {
                                // Identification sent to peer
                            }
                            identify::Event::Pushed { .. } => {
                                // Identification pushed to peer
                            }
                            identify::Event::Error { peer_id, error, .. } => {
                                tracing::warn!(%peer_id, %error, "identify protocol error");
                            }
                        }
                    }

                    // ── ping ──
                    SwarmEvent::Behaviour(ShardBehaviourEvent::Ping(event)) => {
                        let _ = event;
                        // ping events - simplified for compatibility
                        tracing::debug!("ping event: {:?}", event);
                    }

                    // ── mdns ──
                    SwarmEvent::Behaviour(ShardBehaviourEvent::Mdns(event)) => {
                        match event {
                            libp2p::mdns::Event::Discovered(list) => {
                                for (peer_id, multiaddr) in list {
                                    tracing::info!(%peer_id, "mDNS discovered peer");
                                    swarm.behaviour_mut().kad.add_address(&peer_id, multiaddr);
                                }
                            }
                            libp2p::mdns::Event::Expired(list) => {
                                for (peer_id, _multiaddr) in list {
                                    tracing::debug!(%peer_id, "mDNS peer expired");
                                }
                            }
                        }
                    }

                    // ── new listen addresses → update topology ──
                    SwarmEvent::NewListenAddr { address, .. } => {
                        tracing::info!(%address, "listening on");
                        let addr_str = address.to_string();
                        let mut topo = state.topology.lock().await;
                        if !topo.listen_addrs.iter().any(|a| a == &addr_str) {
                            topo.listen_addrs.push(addr_str.clone());
                        }

                        if addr_str.contains("/ws") {
                            topo.ws_addr = Some(format!("{}/p2p/{}", addr_str, local_peer_id));
                        }
                        if addr_str.contains("/webrtc-direct/") {
                            topo.webrtc_addr = Some(format!("{}/p2p/{}", addr_str, local_peer_id));
                        }
                        if addr_str.contains("/quic-v1") {
                            topo.quic_addr = Some(format!("{}/p2p/{}", addr_str, local_peer_id));
                        }
                        let (webrtc_addr, quic_addr, ws_addr, listen_addrs) =
                            outward_topology_addrs(&topo, &state);

                        let topo_json = serde_json::json!({
                            "shard_peer_id": topo.local_peer_id,
                            "shard_webrtc_multiaddr": webrtc_addr,
                            "shard_quic_multiaddr": quic_addr,
                            "shard_ws_multiaddr": ws_addr,
                            "listen_addrs": listen_addrs,
                            "public_api": topo.is_public,
                            "public_api_addr": state.public_host.clone().or(topo.public_api_addr.clone()),
                            "relay_server": topo.relay_server_enabled,
                            "contribute": topo.contribute_enabled,
                            "capacity": topo.capacity,
                            "load": topo.load,
                            "latency_ms": topo.latency_ms,
                        });
                        let _ = tokio::fs::write(&topo_path, topo_json.to_string()).await;
                    }

                    // ── peer connections ──
                    SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                        let should_reject = {
                            let mut penalties = state.scout_penalties.lock().await;
                            penalties.is_blackholed(&peer_id.to_string())
                        };
                        if should_reject {
                            tracing::warn!(%peer_id, "rejecting blackholed peer at transport layer");
                            let _ = swarm.disconnect_peer_id(peer_id);
                            continue;
                        }

                        tracing::info!(%peer_id, "peer connected");
                        reconnect_backoff.remove(&peer_id.to_string());
                        {
                            let mut failures = state.bootstrap_failures.lock().await;
                            failures.remove(&peer_id.to_string());
                        }
                        let remote_addr = endpoint.get_remote_address().to_string();
                        if let Some(policy) = state.network_policy.as_ref() {
                            if let Some(peer_ip) = ip_from_addr_str(remote_addr.as_str()) {
                                match policy.check_connection(peer_ip) {
                                    PolicyDecision::Allow => {}
                                    PolicyDecision::Deny(reason) => {
                                        if policy.audit_log_blocked_connections {
                                            tracing::warn!(
                                                %peer_id,
                                                %peer_ip,
                                                %reason,
                                                "blocked incoming connection by network policy"
                                            );
                                        }
                                        let _ = swarm.disconnect_peer_id(peer_id);
                                        continue;
                                    }
                                }
                            }
                        }
                        let transport_kind = transport_kind_from_text(remote_addr.as_str());
                        record_transport_success(&state.system_metrics, transport_kind);

                        {
                            let mut peers = state.peers.lock().await;
                            let now = now_ms();
                            let entry = peers.entry(peer_id.to_string()).or_insert_with(|| PeerInfo {
                                peer_id: peer_id.to_string(),
                                connected_at: now,
                                last_seen_at: now,
                                addrs: vec![],
                                verified: false,
                                handshake_failures: 0,
                                first_seen_at: now,
                                successful_handshakes: 0,
                                avg_latency_ms: 0.0,
                                connection_failures: 0,
                            });
                            entry.connected_at = now;
                            entry.last_seen_at = now;
                            entry.addrs.push(remote_addr.clone());
                            entry.addrs = unique_addrs(entry.addrs.clone());
                            entry.connection_failures = 0;
                        }

                        if let Ok(remote_multiaddr) = remote_addr.parse::<Multiaddr>() {
                            if is_reconnect_candidate_addr(
                                &remote_multiaddr,
                                allow_private_bootstrap,
                            ) {
                                let mut known = state.known_peers.lock().await;
                                known.push(remote_addr);
                                *known = unique_addrs(known.clone());
                                save_persisted_peers(&known_peers_path, &known).await;
                            }
                        }

                        let req = Heartbeat { kind: "PING".into(), sent_at_ms: now_ms() };
                        let id = swarm.behaviour_mut().handshake.send_request(&peer_id, req);
                        pending_handshakes.insert(id, peer_id);
                    }

                    SwarmEvent::ConnectionClosed { peer_id, num_established, .. } => {
                        // Only act when the *last* connection to this peer closes
                        if num_established > 0 {
                            continue;
                        }
                        tracing::info!(%peer_id, "peer disconnected (last connection)");
                        {
                            let mut peers = state.peers.lock().await;
                            peers.remove(&peer_id.to_string());
                        }

                        // ── Fast reconnect with exponential backoff ──
                        // For known peers, attempt immediate reconnection instead of
                        // waiting for the next 20s reconnect tick.
                        let mut known_addrs: Vec<String> = {
                            let known = state.known_peers.lock().await;
                            known
                                .iter()
                                .filter(|a| a.contains(&peer_id.to_string()))
                                .cloned()
                                .collect()
                        };
                        known_addrs.sort_by_key(|addr| reconnect_addr_sort_key(addr));
                        let connected_snapshot: HashSet<String> =
                            state.peers.lock().await.keys().cloned().collect();
                        if !known_addrs.is_empty() {
                            let peer_ref = peer_id.to_string();
                            let now = now_ms();
                            let mut attempted = false;
                            for addr_str in known_addrs {
                                let Ok(addr) = addr_str.parse::<libp2p::Multiaddr>() else {
                                    continue;
                                };
                                if !should_attempt_reconnect(&addr, &local_peer_id, &connected_snapshot)
                                {
                                    continue;
                                }
                                if let Some((failures, next_eligible)) = reconnect_backoff.get(&peer_ref)
                                {
                                    if now < *next_eligible {
                                        tracing::debug!(
                                            peer_id = %peer_ref,
                                            failures = *failures,
                                            retry_in_ms = (*next_eligible - now),
                                            "fast reconnect deferred (exponential backoff)"
                                        );
                                        break;
                                    }
                                }
                                attempted = true;
                                match swarm.dial(addr.clone()) {
                                    Ok(()) => {
                                        reconnect_backoff.remove(&peer_ref);
                                        tracing::info!(
                                            peer_id = %peer_ref,
                                            %addr,
                                            "fast reconnect dial attempted"
                                        );
                                    }
                                    Err(err) => {
                                        tracing::debug!(
                                            peer_id = %peer_ref,
                                            %addr,
                                            %err,
                                            "fast reconnect dial failed"
                                        );
                                        let entry =
                                            reconnect_backoff.entry(peer_ref.clone()).or_insert((0, 0));
                                        entry.0 += 1;
                                        let backoff_ms = reconnect_backoff_ms_for_failures(entry.0);
                                        entry.1 = now + backoff_ms;
                                    }
                                }
                                break;
                            }
                            if !attempted {
                                tracing::debug!(
                                    peer_id = %peer_ref,
                                    "no eligible address for fast reconnect"
                                );
                            }
                        }
                    }

                    SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                        tracing::warn!(?peer_id, "outgoing connection error");
                        tracing::debug!(?peer_id, %error, "outgoing connection error details");
                        let error_text = error.to_string();
                        let transport_kind = transport_kind_from_text(error_text.as_str());
                        record_transport_failure(&state.system_metrics, transport_kind);

                        // Track bootstrap peer failures
                        if let Some(peer_id) = peer_id {
                            let mut known = state.known_peers.lock().await;
                            let mut failures = state.bootstrap_failures.lock().await;
                            let removed = record_bootstrap_failure(&mut known, &mut failures, &peer_id);
                            if removed {
                                tracing::warn!(%peer_id, "bootstrap peer removed due to too many failures");
                            } else if let Some(count) = failures.get(&peer_id.to_string()) {
                                tracing::warn!(%peer_id, failures = *count, "bootstrap peer connection failure");
                            }
                        }
                    }

                    _ => {}
                }
            }
        }
    }

    // Clean up GPU resources before exit.
    {
        let mut engine_guard = state.engine.lock().await;
        *engine_guard = None;
    }
    Ok(())
}

/// Run the daemon until it completes or a stop signal is received.
///
/// Dropping the `stop` sender will also cause this to return cleanly,
/// which makes it easy to cancel from an external `DaemonTask`.
pub async fn run_until_stopped(
    args: Vec<String>,
    stop: tokio::sync::oneshot::Receiver<()>,
) -> anyhow::Result<()> {
    tokio::select! {
        res = run(args) => res,
        _ = stop => Ok(()),
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::{
        accept_replay_nonce, bootstrap_registry_seed_addrs, filter_bootstrap_addrs,
        is_newer_version, is_non_public_bootstrap_addr, load_bootstrap_registry, model_local_path,
        node_is_healthy, parse_hardcoded_bootstrap_mode, peer_id_from_addr_str,
        prune_bootstrap_registry, record_bootstrap_failure, remove_known_addrs_for_peers,
        save_bootstrap_registry, should_attempt_reconnect, should_include_hardcoded_bootstrap,
        should_reject_peer_connection, should_track_in_flight_path, track_in_flight_path,
        unique_addrs, validate_work_request, BootstrapRegistryEntry, CanaryRolloutConfig,
        CanaryRolloutController, HardcodedBootstrapMode, InFlightRequestGuard, LatencyHistogram,
        ModelManifestEntry, ScoutPenaltyBook, ScoutPenaltyUpdate, ScoutTimeoutTracker,
        SpeculativeConfig, WorkRequest, COLD_BOOTSTRAP_FAILURES, MAX_BOOTSTRAP_FAILURES,
        canonical_bootstrap_multiaddr, reconnect_backoff_ms_for_failures,
    };
    use crate::network::policy::{NetworkMode, NetworkPolicy, PolicyDecision};
    use libp2p::{Multiaddr, PeerId};
    use std::collections::{HashMap, HashSet};
    use std::path::Path;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::sync::Mutex;

    #[test]
    fn unique_addrs_removes_duplicates() {
        let in_addrs = vec![
            "/ip4/127.0.0.1/tcp/4001".to_string(),
            "/ip4/127.0.0.1/tcp/4001".to_string(),
            "/ip4/127.0.0.1/tcp/4101/ws".to_string(),
        ];
        let out = unique_addrs(in_addrs);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn bootstrap_filter_drops_private_addrs_by_default() {
        let peer = PeerId::random();
        let input = vec![
            format!("/ip4/192.168.1.85/tcp/4001/p2p/{peer}"),
            format!("/ip4/35.175.242.222/tcp/4001/p2p/{peer}"),
        ];
        let out = filter_bootstrap_addrs(input, false);
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("35.175.242.222"));
    }

    #[test]
    fn non_public_bootstrap_detection_handles_ipv4_and_ipv6() {
        let private_v4: Multiaddr = "/ip4/192.168.1.10/tcp/4001".parse().unwrap();
        let public_v4: Multiaddr = "/ip4/8.8.8.8/tcp/4001".parse().unwrap();
        let loopback_v6: Multiaddr = "/ip6/::1/tcp/4001".parse().unwrap();
        assert!(is_non_public_bootstrap_addr(&private_v4));
        assert!(!is_non_public_bootstrap_addr(&public_v4));
        assert!(is_non_public_bootstrap_addr(&loopback_v6));
    }

    #[test]
    fn latency_histogram_reports_percentiles() {
        let hist = LatencyHistogram::new();
        for _ in 0..8 {
            hist.observe(10);
        }
        for _ in 0..2 {
            hist.observe(300);
        }

        let p = hist.percentiles();
        assert_eq!(p.samples, 10);
        assert!(p.p50_ms >= 10);
        assert!(p.p90_ms >= 300);
        assert!(p.p99_ms >= p.p90_ms);
    }

    #[test]
    fn latency_histogram_reset_clears_samples() {
        let hist = LatencyHistogram::new();
        hist.observe(25);
        hist.observe(300);
        assert!(hist.percentiles().samples > 0);

        hist.reset();

        let p = hist.percentiles();
        assert_eq!(p.samples, 0);
        assert_eq!(p.p95_ms, 0);
    }

    #[test]
    fn work_request_validation_enforces_bounds() {
        let ok = WorkRequest {
            request_id: "abc".into(),
            prompt_context: "hello".into(),
            min_tokens: 4,
            created_at_ms: None,
            lease_id: None,
            lease_expires_at_ms: None,
            assigned_scout_id: None,
            preferred_endpoint: None,
        };
        assert!(validate_work_request(&ok).is_ok());

        let bad = WorkRequest {
            request_id: "".into(),
            prompt_context: "hello".into(),
            min_tokens: 0,
            created_at_ms: None,
            lease_id: None,
            lease_expires_at_ms: None,
            assigned_scout_id: None,
            preferred_endpoint: None,
        };
        assert!(validate_work_request(&bad).is_err());
    }
    #[test]
    fn test_malicious_scout_blacklist_trigger() {
        let mut penalties = ScoutPenaltyBook::default();
        let peer_id = "PeerID_C".to_string();

        let mut status = penalties.apply_update(
            ScoutPenaltyUpdate {
                peer_id: peer_id.clone(),
                latency_ms: None,
                accepted: true,
                probability_bound: 1.0e-16,
                reason: None,
            },
            0,
        );
        assert!(!status.blackholed);

        for _ in 0..8 {
            status = penalties.apply_update(
                ScoutPenaltyUpdate {
                    peer_id: peer_id.clone(),
                    latency_ms: None,
                    accepted: false,
                    probability_bound: 1.0e-12,
                    reason: Some("poisoned draft".to_string()),
                },
                0,
            );
        }

        assert!(status.score < 55);
        assert!(status.blackholed);
        assert!(penalties.is_blackholed(&peer_id));
    }

    #[test]
    fn test_honest_scout_baseline_reputation() {
        let mut penalties = ScoutPenaltyBook::default();
        let peer_id = "PeerID_A".to_string();
        let mut status = penalties.apply_update(
            ScoutPenaltyUpdate {
                peer_id: peer_id.clone(),
                accepted: true,
                probability_bound: 1.0e-16,
                latency_ms: None,
                reason: None,
            },
            0,
        );

        for _ in 0..9 {
            status = penalties.apply_update(
                ScoutPenaltyUpdate {
                    peer_id: peer_id.clone(),
                    accepted: true,
                    probability_bound: 1.0e-16,
                    latency_ms: None,
                    reason: None,
                },
                0,
            );
        }

        assert_eq!(status.accepted, 10);
        assert_eq!(status.failures, 0);
        assert_eq!(status.score, 100);
        assert!(!status.blackholed);
    }

    #[test]
    fn test_degraded_scout_scoring_without_immediate_ban() {
        let mut penalties = ScoutPenaltyBook::default();
        let peer_id = "PeerID_B".to_string();

        let mut status = penalties.apply_update(
            ScoutPenaltyUpdate {
                peer_id: peer_id.clone(),
                accepted: true,
                probability_bound: 1.0e-16,
                latency_ms: None,
                reason: None,
            },
            0,
        );

        // Mixed quality scout; keep recent sliding success ratio above ban threshold.
        for accepted in [true, false, true, false, true, true, false, true, false] {
            status = penalties.apply_update(
                ScoutPenaltyUpdate {
                    peer_id: peer_id.clone(),
                    accepted,
                    probability_bound: if accepted { 1.0e-16 } else { 1.0e-6 },
                    latency_ms: None,
                    reason: if accepted {
                        None
                    } else {
                        Some("invalid draft".to_string())
                    },
                },
                0,
            );
        }

        assert!(status.score >= 55);
        assert!(!status.blackholed);
    }

    #[test]
    fn test_blacklist_enforcement_rejects_connection() {
        let mut penalties = ScoutPenaltyBook::default();
        let peer_id = "PeerID_C".to_string();

        penalties.apply_update(
            ScoutPenaltyUpdate {
                peer_id: peer_id.clone(),
                accepted: true,
                probability_bound: 1.0e-16,
                latency_ms: None,
                reason: None,
            },
            0,
        );
        for _ in 0..8 {
            penalties.apply_update(
                ScoutPenaltyUpdate {
                    peer_id: peer_id.clone(),
                    accepted: false,
                    probability_bound: 1.0e-12,
                    latency_ms: None,
                    reason: Some("poisoned".to_string()),
                },
                0,
            );
        }

        assert!(should_reject_peer_connection(&mut penalties, &peer_id));
    }

    #[tokio::test]
    async fn replay_nonce_rejects_stale_values() {
        let replay = Arc::new(Mutex::new(HashMap::new()));
        let signer = "abcd".to_string();
        assert!(accept_replay_nonce(&replay, &signer, 1).await);
        assert!(!accept_replay_nonce(&replay, &signer, 1).await);
        assert!(accept_replay_nonce(&replay, &signer, 2).await);
    }

    #[tokio::test]
    async fn replay_nonce_is_monotonic_per_signer_and_isolated_across_signers() {
        let replay = Arc::new(Mutex::new(HashMap::new()));
        let signer_a = "signer-a".to_string();
        let signer_b = "signer-b".to_string();

        assert!(accept_replay_nonce(&replay, &signer_a, 10).await);
        assert!(!accept_replay_nonce(&replay, &signer_a, 9).await);
        assert!(!accept_replay_nonce(&replay, &signer_a, 10).await);
        assert!(accept_replay_nonce(&replay, &signer_b, 10).await);
        assert!(accept_replay_nonce(&replay, &signer_a, 11).await);
    }

    #[test]
    fn node_health_timeout_marks_stale_unhealthy() {
        assert!(node_is_healthy(10_000, 15_000, 5_000));
        assert!(!node_is_healthy(10_000, 20_001, 5_000));
    }

    #[test]
    fn in_flight_tracking_only_counts_compute_paths() {
        for path in [
            "/v1/chat/completions",
            "/ws/generate",
            "/pipeline/forward",
            "/broadcast-work",
            "/signed/broadcast-work",
            "/submit-draft",
            "/v1/scout/draft",
            "/signed/submit-draft",
            "/browser-layer/submit",
        ] {
            assert!(should_track_in_flight_path(path), "{path}");
        }

        for path in [
            "/health",
            "/v1/system/health",
            "/metrics",
            "/metrics/summary",
            "/v1/system/topology",
            "/v1/system/scout-config",
            "/v1/scout/work",
            "/browser-layer/work",
            "/signed/heartbeat",
            "/signed/register-node",
        ] {
            assert!(!should_track_in_flight_path(path), "{path}");
        }
    }

    #[test]
    fn in_flight_guard_releases_counter_on_drop() {
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        {
            let _guard =
                InFlightRequestGuard::new(track_in_flight_path("/v1/chat/completions", &counter));
            assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 1);
        }
        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 0);

        let _guard = InFlightRequestGuard::new(track_in_flight_path("/health", &counter));
        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 0);
    }

    #[test]
    fn reconnect_logic_skips_self_and_connected_peers() {
        let local_peer = PeerId::random();
        let remote_peer = PeerId::random();
        let disconnected_peer = PeerId::random();
        let self_addr: Multiaddr = format!("/ip4/127.0.0.1/tcp/4001/p2p/{local_peer}")
            .parse()
            .unwrap();
        let connected_addr: Multiaddr = format!("/ip4/127.0.0.1/tcp/4001/p2p/{remote_peer}")
            .parse()
            .unwrap();
        let disconnected_addr: Multiaddr =
            format!("/ip4/127.0.0.1/tcp/4001/p2p/{disconnected_peer}")
                .parse()
                .unwrap();

        let mut connected = HashSet::new();
        connected.insert(remote_peer.to_string());

        assert!(!should_attempt_reconnect(
            &self_addr,
            &local_peer,
            &connected
        ));
        assert!(!should_attempt_reconnect(
            &connected_addr,
            &local_peer,
            &connected
        ));
        assert!(should_attempt_reconnect(
            &disconnected_addr,
            &local_peer,
            &connected
        ));
    }

    #[test]
    fn reconnect_sort_key_prefers_public_quic_over_tcp_and_deprioritizes_private_tcp() {
        let peer = PeerId::random();
        let private_tcp = format!("/ip4/192.168.1.25/tcp/4001/p2p/{peer}");
        let public_tcp = format!("/ip4/35.175.242.222/tcp/4001/p2p/{peer}");
        let public_quic = format!("/ip4/35.175.242.222/udp/9092/quic-v1/p2p/{peer}");

        let mut addrs = [public_tcp.clone(), private_tcp.clone(), public_quic.clone()];
        addrs.sort_by_key(|addr| super::reconnect_addr_sort_key(addr));

        assert_eq!(addrs[0], public_quic);
        assert_eq!(addrs[1], public_tcp);
        assert_eq!(addrs[2], private_tcp);
    }

    #[test]
    fn reconnect_candidates_skip_websocket_addrs_for_native_daemon() {
        let peer = PeerId::random();
        let ws = format!("/ip4/35.175.242.222/tcp/4101/ws/p2p/{peer}");
        let quic = format!("/ip4/35.175.242.222/udp/9092/quic-v1/p2p/{peer}");
        let ws_addr = ws.parse::<Multiaddr>().unwrap();
        let quic_addr = quic.parse::<Multiaddr>().unwrap();

        assert!(!super::is_reconnect_candidate_addr(&ws_addr, false));
        assert!(super::is_reconnect_candidate_addr(&quic_addr, false));
    }

    #[test]
    fn reconnect_dial_cap_default_is_eight() {
        assert_eq!(super::max_reconnect_dials_per_tick(), 8);
    }

    #[test]
    fn bootstrap_failures_keep_bootstrap_peer_for_recovery() {
        let peer = PeerId::random();
        let addr = format!("/ip4/127.0.0.1/tcp/4001/p2p/{peer}");
        let mut known = vec![addr];
        let mut failures = HashMap::new();
        let mut removed = false;

        for _ in 0..MAX_BOOTSTRAP_FAILURES {
            removed = record_bootstrap_failure(&mut known, &mut failures, &peer);
        }

        assert!(!removed);
        assert_eq!(known.len(), 1);
        assert_eq!(
            failures.get(&peer.to_string()).copied(),
            Some(MAX_BOOTSTRAP_FAILURES)
        );
    }

    #[test]
    fn bootstrap_failures_eventually_reach_cold_threshold() {
        let peer = PeerId::random();
        let addr = format!("/ip4/127.0.0.1/tcp/4001/p2p/{peer}");
        let mut known = vec![addr];
        let mut failures = HashMap::new();

        for _ in 0..(COLD_BOOTSTRAP_FAILURES + 2) {
            let _ = record_bootstrap_failure(&mut known, &mut failures, &peer);
        }

        assert_eq!(
            failures.get(&peer.to_string()).copied(),
            Some(COLD_BOOTSTRAP_FAILURES)
        );
    }

    #[test]
    fn peer_id_parser_extracts_peer_from_multiaddr_string() {
        let peer = PeerId::random();
        let addr = format!("/ip4/127.0.0.1/tcp/4001/p2p/{peer}");
        assert_eq!(peer_id_from_addr_str(&addr), Some(peer.to_string()));
        assert_eq!(peer_id_from_addr_str("/ip4/127.0.0.1/tcp/4001"), None);
    }

    #[test]
    fn hardcoded_bootstrap_mode_parsing_is_backward_safe() {
        assert_eq!(
            parse_hardcoded_bootstrap_mode(None),
            HardcodedBootstrapMode::Fallback
        );
        assert_eq!(
            parse_hardcoded_bootstrap_mode(Some("always".to_string())),
            HardcodedBootstrapMode::Always
        );
        assert_eq!(
            parse_hardcoded_bootstrap_mode(Some("disabled".to_string())),
            HardcodedBootstrapMode::Disabled
        );
        assert_eq!(
            parse_hardcoded_bootstrap_mode(Some("unknown".to_string())),
            HardcodedBootstrapMode::Fallback
        );
    }

    #[test]
    fn hardcoded_bootstrap_inclusion_respects_mode() {
        assert!(should_include_hardcoded_bootstrap(
            HardcodedBootstrapMode::Always,
            true
        ));
        assert!(should_include_hardcoded_bootstrap(
            HardcodedBootstrapMode::Fallback,
            false
        ));
        assert!(!should_include_hardcoded_bootstrap(
            HardcodedBootstrapMode::Fallback,
            true
        ));
        assert!(!should_include_hardcoded_bootstrap(
            HardcodedBootstrapMode::Disabled,
            false
        ));
    }

    #[test]
    fn canary_rollout_hash_splits_deterministically() {
        let cfg = CanaryRolloutConfig {
            enabled: true,
            canary_model_id: "verifier-v2".to_string(),
            traffic_percent: 50,
            max_avg_latency_ms: 2500,
            min_acceptance_rate: 0.6,
            max_reject_rate: 0.4,
            min_samples: 10,
        };
        let controller = CanaryRolloutController::new("meta-llama/Llama-3.2-1B".to_string(), cfg);
        let decision_a = controller.decide("req-1", true);
        let decision_b = controller.decide("req-1", true);
        assert_eq!(decision_a.use_canary, decision_b.use_canary);
    }

    #[test]
    fn canary_rollout_auto_rollback_on_latency_regression() {
        let cfg = CanaryRolloutConfig {
            enabled: true,
            canary_model_id: "verifier-v2".to_string(),
            traffic_percent: 100,
            max_avg_latency_ms: 10,
            min_acceptance_rate: 0.6,
            max_reject_rate: 0.4,
            min_samples: 3,
        };
        let mut controller =
            CanaryRolloutController::new("meta-llama/Llama-3.2-1B".to_string(), cfg);
        let decision = controller.decide("req-rollback", true);
        for _ in 0..3 {
            controller.record_request_outcome(&decision, 50, Some(0.9), Some(0.1));
        }
        let snapshot = controller.snapshot();
        assert!(snapshot.status.rollback_active);
        assert!(snapshot
            .status
            .rollback_reason
            .unwrap_or_default()
            .contains("latency"));
    }

    #[tokio::test]
    async fn bootstrap_registry_persists_and_loads() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bootstrap_registry.json");
        let mut registry = HashMap::new();
        registry.insert(
            "peer-a".to_string(),
            BootstrapRegistryEntry {
                peer_id: "peer-a".to_string(),
                multiaddr: "/ip4/127.0.0.1/tcp/4001/p2p/12D3KooW".to_string(),
                stability_score: 90,
                uptime_hours: 4,
                version: "0.6.2".to_string(),
                role: Some("bootstrap".to_string()),
                capability_tier: Some("cpu_standard".to_string()),
                gpu_available: Some(false),
                accepts_scout_work: Some(false),
                public_api: Some(true),
                public_api_addr: Some("http://127.0.0.1:9091".to_string()),
                updated_at_ms: 42,
            },
        );

        save_bootstrap_registry(path.as_path(), &registry).await;
        let loaded = load_bootstrap_registry(path.as_path()).await;
        assert_eq!(loaded.len(), 1);
        assert_eq!(
            loaded.get("peer-a").map(|entry| entry.stability_score),
            Some(90)
        );
    }

    #[test]
    fn bootstrap_registry_prunes_stale_entries_and_seeds_stable_addrs() {
        let now = 1_000_000u128;
        let ttl_ms = 60_000u128;
        let mut registry = HashMap::new();
        registry.insert(
            "fresh".to_string(),
            BootstrapRegistryEntry {
                peer_id: "fresh".to_string(),
                multiaddr: "/ip4/35.175.242.222/tcp/4001/p2p/12D3KooWfresh".to_string(),
                stability_score: 92,
                uptime_hours: 12,
                version: "0.6.2".to_string(),
                role: Some("bootstrap".to_string()),
                capability_tier: Some("gpu_fast".to_string()),
                gpu_available: Some(true),
                accepts_scout_work: Some(false),
                public_api: Some(true),
                public_api_addr: Some("https://fresh.shardnetwork.live".to_string()),
                updated_at_ms: now - 10_000,
            },
        );
        registry.insert(
            "stale".to_string(),
            BootstrapRegistryEntry {
                peer_id: "stale".to_string(),
                multiaddr: "/ip4/35.175.242.222/tcp/4001/p2p/12D3KooWstale".to_string(),
                stability_score: 95,
                uptime_hours: 24,
                version: "0.6.2".to_string(),
                role: Some("bootstrap".to_string()),
                capability_tier: Some("cpu_slow".to_string()),
                gpu_available: Some(false),
                accepts_scout_work: Some(true),
                public_api: Some(true),
                public_api_addr: Some("https://stale.shardnetwork.live".to_string()),
                updated_at_ms: now - 120_000,
            },
        );

        let removed = prune_bootstrap_registry(&mut registry, now, ttl_ms);
        assert_eq!(removed, vec!["stale".to_string()]);

        let seeded = bootstrap_registry_seed_addrs(&registry, now, ttl_ms, 80);
        assert_eq!(seeded.len(), 1);
        assert!(seeded[0].contains("12D3KooWfresh"));
    }

    #[test]
    fn canonical_bootstrap_multiaddr_appends_missing_peer_id() {
        let peer = "12D3KooWExamplePeer";
        assert_eq!(
            canonical_bootstrap_multiaddr("/ip4/35.175.242.222/tcp/4001", peer),
            format!("/ip4/35.175.242.222/tcp/4001/p2p/{peer}")
        );
        assert_eq!(
            canonical_bootstrap_multiaddr(
                &format!("/ip4/35.175.242.222/tcp/4001/p2p/{peer}"),
                peer
            ),
            format!("/ip4/35.175.242.222/tcp/4001/p2p/{peer}")
        );
    }

    #[test]
    fn remove_known_addrs_for_peers_drops_only_matching_peer_entries() {
        let keep_peer = PeerId::random().to_string();
        let drop_peer = PeerId::random().to_string();
        let mut known = vec![
            format!("/ip4/35.175.242.222/tcp/4001/p2p/{keep_peer}"),
            format!("/ip4/35.175.242.222/tcp/4001/p2p/{drop_peer}"),
            "/ip4/35.175.242.222/tcp/4001".to_string(),
        ];
        let mut stale = HashSet::new();
        stale.insert(drop_peer.clone());

        let removed = remove_known_addrs_for_peers(&mut known, &stale);
        assert_eq!(removed, 1);
        assert!(known.iter().all(|addr| !addr.contains(&drop_peer)));
        assert!(known.iter().any(|addr| addr.contains(&keep_peer)));
    }

    #[test]
    fn reconnect_backoff_demotes_cold_peers_to_hourly_retry() {
        assert_eq!(reconnect_backoff_ms_for_failures(0), 20_000);
        assert_eq!(reconnect_backoff_ms_for_failures(4), 300_000);
        assert_eq!(
            reconnect_backoff_ms_for_failures(COLD_BOOTSTRAP_FAILURES),
            60 * 60 * 1000
        );
        assert_eq!(
            reconnect_backoff_ms_for_failures(COLD_BOOTSTRAP_FAILURES + 5),
            60 * 60 * 1000
        );
    }

    #[test]
    fn mesh_probe_backoff_caps_and_grows() {
        assert_eq!(super::mesh_probe_backoff_ms_for_failures(0), 15_000);
        assert_eq!(super::mesh_probe_backoff_ms_for_failures(1), 30_000);
        assert_eq!(super::mesh_probe_backoff_ms_for_failures(5), 480_000);
        assert_eq!(super::mesh_probe_backoff_ms_for_failures(8), 15 * 60 * 1000);
    }

    #[test]
    fn scout_timeout_triggers_cooldown_after_threshold() {
        let config = SpeculativeConfig {
            scout_timeout_ms: 100,
            scout_cooldown_ms: 60_000,
            max_consecutive_timeouts: 3,
            draft_token_count: 4,
        };
        let mut tracker = ScoutTimeoutTracker::new();
        assert!(!tracker.is_in_cooldown());
        tracker.record_timeout(&config);
        tracker.record_timeout(&config);
        assert!(!tracker.is_in_cooldown());
        tracker.record_timeout(&config);
        assert!(tracker.is_in_cooldown());
    }

    #[test]
    fn scout_timeout_success_resets_counter() {
        let config = SpeculativeConfig {
            scout_timeout_ms: 100,
            scout_cooldown_ms: 60_000,
            max_consecutive_timeouts: 3,
            draft_token_count: 4,
        };
        let mut tracker = ScoutTimeoutTracker::new();
        tracker.record_timeout(&config);
        tracker.record_timeout(&config);
        tracker.record_success();
        tracker.record_timeout(&config);
        assert!(!tracker.is_in_cooldown());
    }

    #[test]
    fn model_version_compare_detects_newer() {
        assert!(is_newer_version("1.0.0", "1.0.1"));
        assert!(is_newer_version("1.2.9", "1.10.0"));
        assert!(!is_newer_version("2.0.0", "1.9.9"));
    }

    #[test]
    fn model_local_path_contains_id_and_version() {
        let entry = ModelManifestEntry {
            id: "bitnet-1.58-3b".to_string(),
            display_name: "BitNet".to_string(),
            version: "1.0.0".to_string(),
            sha256: "abc".to_string(),
            size_bytes: 123,
            download_url: "https://example.com/models/bitnet.bin".to_string(),
            min_vram_gb: 4,
            min_ram_gb: 8,
            roles: vec!["shard".to_string()],
            quantization: "1.58bit".to_string(),
            architecture: "bitnet".to_string(),
            release_notes: "x".to_string(),
        };
        let path = model_local_path(Path::new("C:/tmp"), &entry);
        let display = path.display().to_string().replace('\\', "/");
        assert!(display.contains("bitnet-1.58-3b/1.0.0/bitnet.bin"));
    }

    #[test]
    fn test_private_mode_blocks_public_ip() {
        let policy = NetworkPolicy {
            mode: NetworkMode::Private,
            allowed_peer_cidrs: vec!["10.0.0.0/8".to_string()],
            blocked_peer_cidrs: vec![],
            allowed_bootstrap_addrs: vec![],
            reject_public_ips: true,
            audit_log_blocked_connections: true,
        };
        let decision = policy.check_connection("8.8.8.8".parse().expect("valid ip"));
        assert!(matches!(decision, PolicyDecision::Deny(_)));
    }
}

fn check_tcp_bind(addr: [u8; 4], port: u16) -> Option<String> {
    if port == 0 {
        return None;
    }
    let addr = std::net::Ipv4Addr::from(addr);
    std::net::TcpListener::bind((addr, port))
        .map(|listener| {
            drop(listener);
            None
        })
        .unwrap_or_else(|e| Some(format!("tcp/{port}: {e}")))
}

fn check_udp_bind(addr: [u8; 4], port: u16) -> Option<String> {
    if port == 0 {
        return None;
    }
    let addr = std::net::Ipv4Addr::from(addr);
    std::net::UdpSocket::bind((addr, port))
        .map(|socket| {
            drop(socket);
            None
        })
        .unwrap_or_else(|e| Some(format!("udp/{port}: {e}")))
}

fn preflight_ports(cli: &Cli) -> Result<(), String> {
    let mut conflicts = Vec::new();
    let control_addr = if cli.public_api {
        [0, 0, 0, 0]
    } else {
        [127, 0, 0, 1]
    };
    if let Some(conflict) = check_tcp_bind(control_addr, cli.control_port) {
        conflicts.push(format!("control port {conflict}"));
    }
    if let Some(conflict) = check_tcp_bind(control_addr, cli.telemetry_ws_port) {
        conflicts.push(format!("telemetry ws {conflict}"));
    }
    if let Some(conflict) = check_tcp_bind([0, 0, 0, 0], cli.tcp_port) {
        conflicts.push(format!("p2p tcp {conflict}"));
    }
    if let Some(conflict) = check_tcp_bind([0, 0, 0, 0], cli.tcp_port + 100) {
        conflicts.push(format!("p2p ws {conflict}"));
    }
    if let Some(conflict) = check_udp_bind([0, 0, 0, 0], cli.webrtc_port) {
        conflicts.push(format!("webrtc {conflict}"));
    }
    if let Some(conflict) = check_udp_bind([0, 0, 0, 0], cli.quic_port) {
        conflicts.push(format!("quic {conflict}"));
    }

    if conflicts.is_empty() {
        Ok(())
    } else {
        Err(format!("port preflight failed: {}", conflicts.join(", ")))
    }
}

#[cfg(unix)]
async fn shutdown_signal() -> bool {
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("failed to install SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => true,
        _ = sigterm.recv() => true,
    }
}

#[cfg(windows)]
async fn shutdown_signal() -> bool {
    let mut sigbreak =
        tokio::signal::windows::ctrl_break().expect("failed to install Ctrl-Break handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => true,
        _ = sigbreak.recv() => true,
    }
}

#[cfg(not(any(unix, windows)))]
async fn shutdown_signal() -> bool {
    tokio::signal::ctrl_c().await.is_ok()
}
