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

use anyhow::Result;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::{
    extract::Path as AxumPath,
    extract::Query,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State as AxumState,
    },
    http::{HeaderValue, Method},
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
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, Mutex};
use tower_http::cors::{Any, CorsLayer};

mod common;
mod crypto;
mod gateway;
mod identity;
pub mod inference;
mod ledger;
mod mesh;
mod metrics;
mod network;
mod scheduler;
mod telemetry_ws;
mod verification;
use common::node_config::{NodeRole, NodeRuntimeConfig};
use common::pow_challenge::PowChallengeManager;
use common::signed_envelope::{EnvelopeVerifier, SignedEnvelope};
use crypto::wallet_backup::{export_wallet, import_wallet, verify_backup};
use gateway::fallback::{ActiveRequestState, FallbackConfig};
use gateway::validate_work_request;
use identity::NodeIdentity;
use ledger::state::{ComputeCreditTx, LedgerState};
use ledger::store::LedgerStore;
use ledger::sync::{hash_probe_segments, LedgerSyncRequest, LedgerSyncResponse};
use mesh::race_router::{RaceKey, RaceRouter, RaceSubmitOutcome};
use metrics::alerts::AlertManager;
use metrics::cost::{estimate as estimate_cost, CostEstimateInput};
use metrics::persistence::{MetricsPersistence, PersistedNodeMetricReport};
use metrics::{NodeMetricReport, NodeMetricSnapshot, PrometheusSample, SystemMetrics};
use network::layer_registry::{provider_key, LayerHostAnnouncement, LayerRoutingTable};
use network::obfuscation::{deobfuscate_bytes, obfuscate_bytes, random_nonce};
use network::private_mesh::PrivateMeshRegistry;
use network::tensor_wire::TensorWirePacket;
use scheduler::{
    load_reputation, save_reputation, weighted_select, NodeReputation, NodeSchedulerInput,
};

// ─── CLI ────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug, Clone)]
#[command(name = "shard-daemon", version, about = "Shard P2P Daemon")]
struct Cli {
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
    bootstrap: Vec<String>,

    /// Path to newline-delimited bootstrap multiaddrs
    #[arg(long)]
    bootstrap_file: Option<String>,

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
    relay_server: bool,

    /// Contribute compute to the network (run as Shard node)
    #[arg(long, default_value = "true")]
    contribute: bool,

    /// Enable NAT traversal (circuit relay + hole punching)
    #[arg(long, default_value = "true")]
    nat_traversal: bool,

    /// Hosted model identifier for layer routing announcements.
    #[arg(long, default_value = "default-model")]
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
}

#[derive(Subcommand, Debug, Clone)]
enum DaemonCommand {
    Wallet {
        #[command(subcommand)]
        command: WalletCommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum WalletCommand {
    Show,
    Export(WalletExportArgs),
    Import(WalletImportArgs),
    VerifyBackup(WalletVerifyArgs),
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
struct DraftSubmission {
    task_id: String,
    scout_peer_id: String,
    seq_start: u32,
    draft_tokens: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkRequest {
    pub request_id: String,
    pub prompt_context: String,
    pub min_tokens: i32,
    #[serde(default)]
    pub created_at_ms: Option<u128>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkResponse {
    pub request_id: String,
    pub peer_id: String,
    pub draft_tokens: Vec<String>,
    pub latency_ms: f32,
    #[serde(default)]
    pub created_at_ms: Option<u128>,
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
}

#[derive(Clone)]
struct SharedState {
    topology: Arc<Mutex<TopologyState>>,
    peers: Arc<Mutex<HashMap<String, PeerInfo>>>,
    known_peers: Arc<Mutex<Vec<String>>>,
    results: Arc<Mutex<VecDeque<WorkResponse>>>,
    scout_work: Arc<Mutex<VecDeque<WorkRequest>>>,
    work_tx: mpsc::Sender<WorkRequest>,
    pipeline_tx: mpsc::Sender<PipelineDispatch>,
    browser_result_tx: mpsc::Sender<ForwardPassActivation>,
    daemon_start: u128,
    capacity: Arc<AtomicU32>,
    current_load: Arc<AtomicU32>,
    avg_latency_ms: Arc<AtomicU32>,
    gossipsub_latency_hist: Arc<LatencyHistogram>,
    credit_nonce: Arc<AtomicU64>,
    scout_penalties: Arc<Mutex<ScoutPenaltyBook>>,
    backward_passes: Arc<Mutex<VecDeque<BackwardPassGradient>>>,
    layer_routes: Arc<Mutex<LayerRoutingTable>>,
    race_router: Arc<Mutex<RaceRouter>>,
    ledger: Arc<Mutex<LedgerState>>,
    ledger_store: Arc<LedgerStore>,
    browser_sessions: Arc<Mutex<HashMap<String, BrowserLayerSession>>>,
    browser_work: Arc<Mutex<VecDeque<BrowserLayerWorkItem>>>,
    node_wallet: String,
    model_id: String,
    layer_start: u32,
    layer_end: u32,
    race_pool_size: usize,
    race_timeout_ms: u64,
    engine: Arc<Mutex<Option<crate::inference::ShardEngine>>>,
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
    heartbeat_interval_seconds: u64,
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
}

#[derive(Clone, Debug, Serialize)]
struct ResourcePolicy {
    max_cpu_usage: f32,
    max_gpu_usage: f32,
    idle_only_mode: bool,
    load_threshold_cutoff: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScoutPenaltyUpdate {
    peer_id: String,
    accepted: bool,
    probability_bound: f64,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DraftResultSubmission {
    work_id: String,
    scout_id: String,
    draft_text: String,
    #[serde(default)]
    timestamp: Option<f64>,
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
    last_reason: Option<String>,
}

#[derive(Debug, Default)]
struct ScoutReputationEntry {
    recent: VecDeque<bool>,
    failure_count: u32,
    accepted_count: u32,
    banned_until_ms: Option<u128>,
    last_reason: Option<String>,
}

#[derive(Debug, Default)]
struct ScoutPenaltyBook {
    entries: HashMap<String, ScoutReputationEntry>,
}

impl ScoutPenaltyBook {
    const WINDOW_SIZE: usize = 10;
    const MIN_SAMPLES_FOR_BAN: usize = 5;
    const SUCCESS_RATE_THRESHOLD: f32 = 0.55;
    const BAN_COOLDOWN_MS: u128 = 60_000;

    fn success_rate(entry: &ScoutReputationEntry) -> f32 {
        if entry.recent.is_empty() {
            return 1.0;
        }
        let success = entry.recent.iter().filter(|ok| **ok).count() as f32;
        success / (entry.recent.len() as f32)
    }

    fn apply_update(&mut self, update: ScoutPenaltyUpdate) -> ScoutPenaltyStatus {
        let now = now_ms();
        let entry = self.entries.entry(update.peer_id.clone()).or_default();

        if entry.recent.len() >= Self::WINDOW_SIZE {
            entry.recent.pop_front();
        }
        entry.recent.push_back(update.accepted);

        if update.accepted {
            entry.accepted_count = entry.accepted_count.saturating_add(1);
        } else {
            entry.failure_count = entry.failure_count.saturating_add(1);
            if let Some(reason) = update.reason.as_ref() {
                entry.last_reason = Some(reason.clone());
            }
        }

        let success_rate = Self::success_rate(entry);
        if entry.recent.len() >= Self::MIN_SAMPLES_FOR_BAN
            && success_rate < Self::SUCCESS_RATE_THRESHOLD
        {
            entry.banned_until_ms = Some(now + Self::BAN_COOLDOWN_MS);
        }

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
                p99_ms: 0,
                samples: 0,
            };
        }

        let p50_target = ((total as f64) * 0.50).ceil() as u64;
        let p90_target = ((total as f64) * 0.90).ceil() as u64;
        let p99_target = ((total as f64) * 0.99).ceil() as u64;

        let mut running = 0u64;
        let mut p50 = 0u64;
        let mut p90 = 0u64;
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
            if p99 == 0 && running >= p99_target {
                p99 = bucket_upper;
                break;
            }
        }

        LatencyPercentiles {
            p50_ms: p50,
            p90_ms: p90,
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
    relay_server: relay::Behaviour,
    dcutr: dcutr::Behaviour,
    autonat: autonat::v1::Behaviour,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

fn data_dir() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("shard")
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

fn host_multiaddr_prefix(host: &str) -> (String, String) {
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(_)) => ("ip4".to_string(), host.to_string()),
        Ok(IpAddr::V6(_)) => ("ip6".to_string(), host.to_string()),
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
    let capacity = state.capacity.load(Ordering::Relaxed).max(1);
    let load = state.current_load.load(Ordering::Relaxed);
    let load_ratio = load as f32 / capacity as f32;

    if state.resource_policy.idle_only_mode && load > 0 {
        return Err("idle_only_mode enabled and node is busy".to_string());
    }
    if load_ratio > state.resource_policy.load_threshold_cutoff {
        return Err("load threshold cutoff reached".to_string());
    }
    Ok(())
}

// ─── HTTP Control-Plane Handlers ────────────────────────────────────────────

async fn health_handler(AxumState(state): AxumState<SharedState>) -> Json<serde_json::Value> {
    let topo = state.topology.lock().await;
    let peers = state.peers.lock().await;
    let known = state.known_peers.lock().await;
    let verified_count = peers.values().filter(|p| p.verified).count();
    let capacity = state.capacity.load(Ordering::Relaxed);
    let load = state.current_load.load(Ordering::Relaxed);
    let latency_ms = state.avg_latency_ms.load(Ordering::Relaxed);
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "peer_id": topo.local_peer_id,
        "connected_peers": peers.len(),
        "verified_peers": verified_count,
        "known_peers": known.len(),
        "uptime_ms": now_ms() - state.daemon_start,
        "listen_addrs": topo.listen_addrs,
        "public_api": topo.is_public,
        "public_api_addr": topo.public_api_addr,
        "relay_server": topo.relay_server_enabled,
        "contribute": topo.contribute_enabled,
        "wallet": state.node_wallet.clone(),
        "model_id": state.model_id.clone(),
        "layer_start": state.layer_start,
        "layer_end": state.layer_end,
        "race_pool_size": state.race_pool_size,
        "race_timeout_ms": state.race_timeout_ms,
        "capacity": capacity,
        "load": load,
        "latency_ms": latency_ms,
    }))
}

async fn node_status_handler(AxumState(state): AxumState<SharedState>) -> Json<serde_json::Value> {
    let topo = state.topology.lock().await;
    let logs = state.event_log.lock().await;
    Json(serde_json::json!({
        "ok": true,
        "node_role": state.node_role,
        "node_public_key": state.node_public_key,
        "participation_enabled": state.participation_enabled.load(Ordering::Relaxed),
        "resource_policy": state.resource_policy,
        "current_load": state.current_load.load(Ordering::Relaxed),
        "capacity": state.capacity.load(Ordering::Relaxed),
        "latency_ms": state.avg_latency_ms.load(Ordering::Relaxed),
        "health_status": if state.participation_enabled.load(Ordering::Relaxed) { "healthy" } else { "paused" },
        "peer_id": topo.local_peer_id,
        "recent_logs": logs.iter().cloned().collect::<Vec<String>>(),
    }))
}

async fn node_toggle_participation_handler(
    AxumState(state): AxumState<SharedState>,
    Json(toggle): Json<ParticipationToggle>,
) -> Json<serde_json::Value> {
    state
        .participation_enabled
        .store(toggle.enabled, Ordering::Relaxed);
    append_event_log(
        &state,
        format!("participation toggled to {}", toggle.enabled),
    )
    .await;
    Json(serde_json::json!({
        "ok": true,
        "participation_enabled": toggle.enabled,
    }))
}

async fn node_logs_handler(AxumState(state): AxumState<SharedState>) -> Json<serde_json::Value> {
    let logs = state.event_log.lock().await;
    Json(serde_json::json!({
        "ok": true,
        "logs": logs.iter().cloned().collect::<Vec<String>>(),
    }))
}

async fn node_ui_handler() -> Html<&'static str> {
    Html(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"/><meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>Shard Node UI</title>
<style>
body{font-family:"IBM Plex Sans","Segoe UI",sans-serif;background:#101a26;color:#e8f2ff;margin:0;padding:20px}
.card{background:#162536;border:1px solid #264a6e;border-radius:12px;padding:14px;margin-bottom:12px}
button{background:#2aa7df;color:#021019;border:0;padding:8px 10px;border-radius:8px;font-weight:700;cursor:pointer}
pre{white-space:pre-wrap;max-height:300px;overflow:auto}
</style></head><body>
<h1>Shard Node Control</h1>
<div class="card"><h3>Status</h3><pre id="status">loading...</pre></div>
<div class="card"><button onclick="toggle()">Toggle Participation</button></div>
<div class="card"><h3>Logs</h3><pre id="logs">loading...</pre></div>
<script>
async function refresh(){
  const s = await fetch('/node/status').then(r=>r.json());
  const l = await fetch('/node/logs').then(r=>r.json());
  document.getElementById('status').textContent = JSON.stringify(s,null,2);
  document.getElementById('logs').textContent = (l.logs||[]).join('\n');
}
async function toggle(){
  const s = await fetch('/node/status').then(r=>r.json());
  await fetch('/node/toggle-participation',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({enabled:!s.participation_enabled})});
  await refresh();
}
refresh(); setInterval(refresh, 4000);
</script></body></html>"#,
    )
}

async fn topology_handler(AxumState(state): AxumState<SharedState>) -> Json<serde_json::Value> {
    let topo = state.topology.lock().await;
    let (webrtc_addr, quic_addr, ws_addr, listen_addrs) = outward_topology_addrs(&topo, &state);
    let known = state.known_peers.lock().await;
    let capacity = state.capacity.load(Ordering::Relaxed);
    let load = state.current_load.load(Ordering::Relaxed);
    let latency_ms = state.avg_latency_ms.load(Ordering::Relaxed);
    Json(serde_json::json!({
        "status": "ok",
        "source": "rust-sidecar",
        "shard_peer_id": topo.local_peer_id,
        "shard_webrtc_multiaddr": webrtc_addr,
        "shard_quic_multiaddr": quic_addr,
        "shard_ws_multiaddr": ws_addr,
        "listen_addrs": listen_addrs,
        "known_peer_count": known.len(),
        "public_api": topo.is_public,
        "public_api_addr": state.public_host.clone().or(topo.public_api_addr.clone()),
        "relay_server": topo.relay_server_enabled,
        "contribute": topo.contribute_enabled,
        "wallet": state.node_wallet.clone(),
        "model_id": state.model_id.clone(),
        "layer_start": state.layer_start,
        "layer_end": state.layer_end,
        "race_pool_size": state.race_pool_size,
        "race_timeout_ms": state.race_timeout_ms,
        "capacity": capacity,
        "load": load,
        "latency_ms": latency_ms,
    }))
}

async fn peers_handler(AxumState(state): AxumState<SharedState>) -> Json<serde_json::Value> {
    let peers = state.peers.lock().await;
    let list: Vec<&PeerInfo> = peers.values().collect();
    Json(serde_json::json!({ "peers": list, "count": list.len() }))
}

const BROWSER_SESSION_TTL_MS: u128 = 5 * 60 * 1000;

fn parse_nonce_hex(raw: &str) -> Result<[u8; 12], String> {
    let bytes = hex::decode(raw).map_err(|e| format!("invalid nonce hex: {e}"))?;
    if bytes.len() != 12 {
        return Err("nonce must be 12 bytes".into());
    }
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&bytes);
    Ok(nonce)
}

fn prune_browser_sessions(sessions: &mut HashMap<String, BrowserLayerSession>, now: u128) {
    sessions.retain(|_, session| session.expires_at_ms > now);
}

async fn credits_balance_handler(
    AxumPath(wallet): AxumPath<String>,
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let ledger = state.ledger.lock().await;
    let balance = ledger.balance_of(wallet.as_str());
    Json(serde_json::json!({
        "ok": true,
        "wallet": wallet,
        "balance": balance,
    }))
}

async fn wallet_address_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "ok": true,
        "wallet": state.node_wallet.clone(),
    }))
}

async fn credits_tx_handler(
    AxumPath(tx_id): AxumPath<String>,
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let ledger = state.ledger.lock().await;
    match ledger.tx_by_id(tx_id.as_str()) {
        Some(tx) => Json(serde_json::json!({ "ok": true, "tx": tx })),
        None => Json(serde_json::json!({ "ok": false, "detail": "transaction not found" })),
    }
}

async fn ledger_head_handler(AxumState(state): AxumState<SharedState>) -> Json<serde_json::Value> {
    let ledger = state.ledger.lock().await;
    Json(serde_json::json!({
        "ok": true,
        "head": ledger.head(),
    }))
}

async fn ledger_stats_handler(AxumState(state): AxumState<SharedState>) -> Json<serde_json::Value> {
    let ledger = state.ledger.lock().await;
    Json(serde_json::json!({
        "ok": true,
        "stats": ledger.stats(),
    }))
}

async fn ledger_export_handler(
    AxumState(state): AxumState<SharedState>,
    Query(query): Query<LedgerExportQuery>,
) -> Json<serde_json::Value> {
    let from_height = query.from_height.unwrap_or(1);
    let limit = query.limit.unwrap_or(512).clamp(1, 4096);
    let ledger = state.ledger.lock().await;
    let export = ledger.export_range(from_height, limit);
    Json(serde_json::json!({
        "ok": true,
        "export": export,
    }))
}

async fn next_layer_handler(
    AxumState(state): AxumState<SharedState>,
    Query(query): Query<NextLayerQuery>,
) -> Json<serde_json::Value> {
    let model_id = query
        .model_id
        .as_deref()
        .unwrap_or(state.model_id.as_str())
        .to_string();
    let limit = query.limit.unwrap_or(3).clamp(1, 16);
    let mut routes = state.layer_routes.lock().await;
    routes.prune_expired(now_ms());
    let peers = routes.find_next_layer_peers(&model_id, query.current_layer, limit);
    drop(routes);

    let snapshots = state.node_metric_reports.lock().await.clone();
    let reputation = state.node_reputation.lock().await.clone();
    let scheduler_inputs = peers
        .iter()
        .map(|peer_id| {
            let snapshot = snapshots.get(peer_id);
            let rep = reputation.get(peer_id);
            NodeSchedulerInput {
                node_id: peer_id.clone(),
                load: snapshot
                    .map(|s| (s.queue_depth as f64 / 64.0).min(1.0))
                    .unwrap_or(0.25),
                latency_ms: snapshot.map(|s| s.node_latency_ms as f64).unwrap_or(250.0),
                reliability_score: rep.map(NodeReputation::reliability_score).unwrap_or(0.8),
                hardware_capability_score: rep
                    .map(|r| r.hardware_capability_score)
                    .filter(|v| *v > 0.0)
                    .unwrap_or(0.7),
                identity_reputation_score: rep.map(|r| r.identity_score).unwrap_or(0.75),
            }
        })
        .collect::<Vec<_>>();
    let selected = weighted_select(scheduler_inputs, limit);

    Json(serde_json::json!({
        "ok": true,
        "model_id": model_id,
        "current_layer": query.current_layer,
        "next_layer": query.current_layer.saturating_add(1),
        "peers": selected,
        "count": selected.len(),
    }))
}

async fn pipeline_forward_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<PipelineForwardRequest>,
) -> Json<serde_json::Value> {
    let model_id = req
        .model_id
        .as_deref()
        .unwrap_or(state.model_id.as_str())
        .to_string();
    let dispatch = PipelineDispatch {
        model_id,
        current_layer: req.current_layer,
        packet: req.packet,
    };
    match state.pipeline_tx.send(dispatch).await {
        Ok(_) => Json(serde_json::json!({
            "ok": true,
            "detail": "queued for pooled forward dispatch"
        })),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "detail": format!("channel error: {e}")
        })),
    }
}

async fn pipeline_pop_forward_result_handler(
    AxumState(state): AxumState<SharedState>,
    Query(query): Query<ForwardResultQuery>,
) -> Json<serde_json::Value> {
    let key = match (&query.request_id, &query.step_id) {
        (Some(r), Some(s)) => Some(RaceKey {
            request_id: r.clone(),
            step_id: s.clone(),
        }),
        _ => None,
    };

    let result = {
        let mut router = state.race_router.lock().await;
        router.pop_completed(key.as_ref())
    };

    match result {
        Some(res) => Json(serde_json::json!({ "ok": true, "result": res })),
        None => {
            Json(serde_json::json!({ "ok": false, "detail": "no completed race result available" }))
        }
    }
}

async fn browser_layer_register_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<BrowserLayerRegisterRequest>,
) -> Json<serde_json::Value> {
    if req.layer_end < req.layer_start {
        return Json(serde_json::json!({
            "ok": false,
            "detail": "layer_end must be >= layer_start",
        }));
    }
    if !req.profile.supports_webgpu {
        return Json(serde_json::json!({
            "ok": false,
            "detail": "webgpu support is required for browser layer hosting",
        }));
    }

    let model_id = req
        .model_id
        .as_deref()
        .unwrap_or(state.model_id.as_str())
        .to_string();
    let now = now_ms();
    let session_id = uuid::Uuid::new_v4().to_string();
    let obfuscation_key = rand::random::<[u8; 32]>().to_vec();
    let expires_at_ms = now + BROWSER_SESSION_TTL_MS;
    let session = BrowserLayerSession {
        session_id: session_id.clone(),
        model_id: model_id.clone(),
        layer_start: req.layer_start,
        layer_end: req.layer_end,
        profile: req.profile,
        obfuscation_key: obfuscation_key.clone(),
        last_seen_ms: now,
        expires_at_ms,
    };
    {
        let mut sessions = state.browser_sessions.lock().await;
        prune_browser_sessions(&mut sessions, now);
        sessions.insert(session_id.clone(), session);
    }

    Json(serde_json::json!(BrowserLayerRegisterResponse {
        ok: true,
        session_id,
        model_id,
        layer_start: req.layer_start,
        layer_end: req.layer_end,
        expires_at_ms,
        obfuscation_key_hex: hex::encode(obfuscation_key),
    }))
}

async fn browser_layer_work_handler(
    AxumState(state): AxumState<SharedState>,
    Query(query): Query<BrowserLayerWorkQuery>,
) -> Json<serde_json::Value> {
    let now = now_ms();
    {
        let mut sessions = state.browser_sessions.lock().await;
        prune_browser_sessions(&mut sessions, now);
        let Some(session) = sessions.get_mut(query.session_id.as_str()) else {
            return Json(serde_json::json!({"ok": false, "detail": "invalid session_id"}));
        };
        session.last_seen_ms = now;
        session.expires_at_ms = now + BROWSER_SESSION_TTL_MS;
    }

    let mut queue = state.browser_work.lock().await;
    if let Some(pos) = queue
        .iter()
        .position(|item| item.session_id == query.session_id)
    {
        if let Some(work) = queue.remove(pos) {
            return Json(serde_json::json!({
                "ok": true,
                "work": work,
            }));
        }
    }
    Json(serde_json::json!({"ok": true, "status": "empty"}))
}

async fn browser_layer_submit_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<BrowserLayerResultSubmit>,
) -> Json<serde_json::Value> {
    let key = {
        let mut sessions = state.browser_sessions.lock().await;
        prune_browser_sessions(&mut sessions, now_ms());
        let Some(session) = sessions.get_mut(req.session_id.as_str()) else {
            return Json(serde_json::json!({"ok": false, "detail": "invalid session_id"}));
        };
        session.last_seen_ms = now_ms();
        session.expires_at_ms = now_ms() + BROWSER_SESSION_TTL_MS;
        session.obfuscation_key.clone()
    };
    let topo_peer_id = state.topology.lock().await.local_peer_id.clone();

    let nonce = match parse_nonce_hex(req.nonce_hex.as_str()) {
        Ok(nonce) => nonce,
        Err(detail) => return Json(serde_json::json!({"ok": false, "detail": detail})),
    };
    let cipher = match hex::decode(req.obfuscated_tensor_hex.as_str()) {
        Ok(raw) => raw,
        Err(e) => {
            return Json(serde_json::json!({
                "ok": false,
                "detail": format!("invalid obfuscated_tensor_hex: {e}"),
            }))
        }
    };
    let plain = deobfuscate_bytes(&key, &nonce, &cipher);
    let wire = TensorWirePacket {
        tensor_name: req.tensor_name.clone(),
        dtype: req.dtype,
        shape: req.shape.clone(),
        data: plain,
    };
    let encoded = wire.encode();
    let response_packet = ForwardPassActivation {
        request_id: req.request_id.clone(),
        step_id: req.step_id.clone(),
        source_peer_id: topo_peer_id,
        target_peer_id: Some(req.source_peer_id.clone()),
        target_peer_pool: None,
        tensor_name: req.tensor_name,
        shape: req.shape.iter().map(|v| *v as usize).collect(),
        format: TensorDataFormat::Quantized,
        chunk: Some(TensorChunkRef {
            chunk_index: 0,
            total_chunks: 1,
            byte_size: encoded.len() as u64,
            checksum_blake3: None,
            data: format!("wire1:{}", hex::encode(encoded)),
        }),
        blob_ref: None,
        created_at_ms: Some(now_ms()),
    };

    match state.browser_result_tx.send(response_packet).await {
        Ok(_) => Json(serde_json::json!({
            "ok": true,
            "detail": "accepted browser activation result",
        })),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "detail": format!("failed to enqueue browser activation result: {e}"),
        })),
    }
}

async fn broadcast_work_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<WorkRequest>,
) -> Json<serde_json::Value> {
    process_work_request(&state, req).await
}

async fn signed_broadcast_work_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<SignedRequest<WorkRequest>>,
) -> Json<serde_json::Value> {
    let signer = req.envelope.signer_pubkey_hex.clone();
    if let Err(detail) = req.envelope.verify() {
        state.system_metrics.inc_signature_verification_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": detail }));
    }
    if !accept_replay_nonce(
        &state.replay_nonces,
        &req.envelope.signer_pubkey_hex,
        req.envelope.nonce,
    )
    .await
    {
        state.system_metrics.inc_node_identity_auth_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": "stale or replayed nonce" }));
    }
    process_work_request(&state, req.envelope.payload).await
}

async fn process_work_request(state: &SharedState, req: WorkRequest) -> Json<serde_json::Value> {
    if let Err(detail) = validate_work_request(&req) {
        state.system_metrics.inc_task_failures();
        return Json(serde_json::json!({ "ok": false, "detail": detail }));
    }
    if let Err(detail) = should_accept_work(state) {
        state.system_metrics.inc_task_failures();
        return Json(serde_json::json!({ "ok": false, "detail": detail }));
    }

    {
        let mut queue = state.scout_work.lock().await;
        queue.push_back(req.clone());
        while queue.len() > 1024 {
            queue.pop_front();
        }
    }

    match state.work_tx.send(req).await {
        Ok(_) => Json(serde_json::json!({ "ok": true, "detail": "queued for gossipsub publish" })),
        Err(e) => {
            state.system_metrics.inc_task_failures();
            Json(serde_json::json!({ "ok": false, "detail": format!("channel error: {e}") }))
        }
    }
}

async fn pop_result_handler(
    AxumState(state): AxumState<SharedState>,
    Query(query): Query<PopResultQuery>,
) -> Json<serde_json::Value> {
    if let Some(request_id) = query.request_id.clone() {
        let mut by_id = state.idempotent_results.lock().await;
        if let Some(result) = by_id.remove(&request_id) {
            state
                .system_metrics
                .inc_tokens_processed(result.draft_tokens.len() as u64);
            return Json(serde_json::json!({ "result": result }));
        }
    }

    let mut results = state.results.lock().await;
    if let Some(request_id) = query.request_id {
        if let Some(idx) = results.iter().position(|r| r.request_id == request_id) {
            if let Some(result) = results.remove(idx) {
                state
                    .system_metrics
                    .inc_tokens_processed(result.draft_tokens.len() as u64);
                return Json(serde_json::json!({ "result": result }));
            }
        }
        return Json(serde_json::json!({ "result": null }));
    }

    match results.pop_front() {
        Some(result) => {
            state
                .system_metrics
                .inc_tokens_processed(result.draft_tokens.len() as u64);
            Json(serde_json::json!({ "result": result }))
        }
        None => Json(serde_json::json!({ "result": null })),
    }
}

async fn pop_work_handler(AxumState(state): AxumState<SharedState>) -> Json<serde_json::Value> {
    let mut queue = state.scout_work.lock().await;
    match queue.pop_front() {
        Some(work) => Json(serde_json::json!({ "work": work })),
        None => Json(serde_json::json!({ "work": null })),
    }
}

async fn submit_draft_handler(
    AxumState(state): AxumState<SharedState>,
    Json(submission): Json<DraftResultSubmission>,
) -> Json<serde_json::Value> {
    process_draft_submission(&state, submission).await
}

async fn signed_submit_draft_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<SignedRequest<DraftResultSubmission>>,
) -> Json<serde_json::Value> {
    let signer = req.envelope.signer_pubkey_hex.clone();
    if let Err(detail) = req.envelope.verify() {
        state.system_metrics.inc_signature_verification_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": detail }));
    }
    if !accept_replay_nonce(
        &state.replay_nonces,
        &req.envelope.signer_pubkey_hex,
        req.envelope.nonce,
    )
    .await
    {
        state.system_metrics.inc_node_identity_auth_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": "stale or replayed nonce" }));
    }
    process_draft_submission(&state, req.envelope.payload).await
}

async fn process_draft_submission(
    state: &SharedState,
    submission: DraftResultSubmission,
) -> Json<serde_json::Value> {
    if submission.work_id.trim().is_empty() || submission.scout_id.trim().is_empty() {
        state.system_metrics.inc_task_failures();
        if !submission.scout_id.trim().is_empty() {
            mark_node_failure(state, submission.scout_id.as_str()).await;
        }
        return Json(serde_json::json!({
            "ok": false,
            "detail": "work_id and scout_id are required",
        }));
    }

    let created_at_ms = submission
        .timestamp
        .map(|ts| (ts * 1000.0).max(0.0) as u128)
        .unwrap_or_else(now_ms);

    let response = WorkResponse {
        request_id: submission.work_id,
        peer_id: submission.scout_id,
        draft_tokens: submission
            .draft_text
            .split_whitespace()
            .map(ToOwned::to_owned)
            .collect(),
        latency_ms: 0.0,
        created_at_ms: Some(created_at_ms),
    };

    {
        let mut by_id = state.idempotent_results.lock().await;
        if by_id.contains_key(response.request_id.as_str()) {
            mark_node_success(state, response.peer_id.as_str(), 0.0).await;
            return Json(serde_json::json!({
                "ok": true,
                "detail": "duplicate draft ignored (idempotent)",
            }));
        }
        by_id.insert(response.request_id.clone(), response.clone());
    }

    state
        .system_metrics
        .inc_tokens_offloaded_to_scouts(response.draft_tokens.len() as u64);

    let mut results = state.results.lock().await;
    results.push_back(response.clone());
    while results.len() > 2048 {
        results.pop_front();
    }

    mark_node_success(state, response.peer_id.as_str(), response.latency_ms as f64).await;

    Json(serde_json::json!({ "ok": true, "detail": "draft queued" }))
}

async fn accept_replay_nonce(
    replay_nonces: &Arc<Mutex<HashMap<String, u64>>>,
    signer_pubkey_hex: &str,
    nonce: u64,
) -> bool {
    let mut guard = replay_nonces.lock().await;
    let previous = guard.get(signer_pubkey_hex).copied().unwrap_or(0);
    if nonce <= previous {
        return false;
    }
    guard.insert(signer_pubkey_hex.to_string(), nonce);
    true
}

async fn generate_local_fallback_tokens(
    state: &SharedState,
    prompt_context: &str,
    max_new_tokens: u32,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut engine_guard = state.engine.lock().await;
    let Some(engine) = engine_guard.as_mut() else {
        return out;
    };

    let Ok(mut tokens) = engine.tokenize(prompt_context, 4096) else {
        return out;
    };
    if !tokens.is_empty() && tokens[0] == 128000 {
        tokens.remove(0);
    }
    if engine.eval(&tokens).is_err() {
        return out;
    }

    let mut emitted = 0u32;
    while emitted < max_new_tokens {
        let Ok(logits) = engine.get_logits(128256) else {
            break;
        };

        let mut best_idx = 0usize;
        let mut best_val = -f32::INFINITY;
        for (idx, val) in logits.iter().enumerate() {
            if *val > best_val {
                best_val = *val;
                best_idx = idx;
            }
        }
        if best_idx == 128001 || best_idx == 128009 {
            break;
        }
        if let Ok(piece) = engine.token_to_piece(best_idx as i32) {
            out.push(piece);
        }
        if engine.eval(&[best_idx as i32]).is_err() {
            break;
        }
        emitted += 1;
    }
    out
}

async fn ws_generate_handler(
    ws: WebSocketUpgrade,
    AxumState(state): AxumState<SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_generate_stream(socket, state))
}

async fn ws_generate_stream(mut socket: WebSocket, state: SharedState) {
    let first = socket.recv().await;
    let Some(Ok(Message::Text(payload))) = first else {
        return;
    };

    let parsed: WsGenerateRequest = match serde_json::from_str(&payload) {
        Ok(req) => req,
        Err(err) => {
            let _ = socket
                .send(Message::Text(
                    serde_json::json!({"error": format!("invalid request: {err}")}).to_string(),
                ))
                .await;
            return;
        }
    };

    let request_id = parsed
        .request_id
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| format!("ws-{}", uuid::Uuid::new_v4()));
    let prompt_context = parsed
        .prompt
        .or(parsed.prompt_context)
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_default();
    let max_new_tokens = parsed.max_new_tokens.unwrap_or(128).clamp(1, 2048);

    let work = WorkRequest {
        request_id: request_id.clone(),
        prompt_context: prompt_context.clone(),
        min_tokens: 1,
        created_at_ms: Some(now_ms()),
    };

    if let Err(detail) = validate_work_request(&work) {
        state.system_metrics.inc_task_failures();
        let _ = socket
            .send(Message::Text(
                serde_json::json!({"error": detail, "event": "done"}).to_string(),
            ))
            .await;
        return;
    }
    if let Err(detail) = should_accept_work(&state) {
        state.system_metrics.inc_task_failures();
        let _ = socket
            .send(Message::Text(
                serde_json::json!({"error": detail, "event": "done"}).to_string(),
            ))
            .await;
        return;
    }

    {
        let mut queue = state.scout_work.lock().await;
        queue.push_back(work.clone());
        while queue.len() > 1024 {
            queue.pop_front();
        }
    }

    if state.work_tx.send(work).await.is_err() {
        state.system_metrics.inc_task_failures();
        state.system_metrics.inc_verification_fallback();
        let _ = socket
            .send(Message::Text(
                serde_json::json!({"error": "work queue unavailable", "event": "done"}).to_string(),
            ))
            .await;
        return;
    }

    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let mut emitted = 0u32;
    while emitted < max_new_tokens && tokio::time::Instant::now() < deadline {
        let maybe_result = {
            let mut results = state.results.lock().await;
            if let Some(idx) = results.iter().position(|r| r.request_id == request_id) {
                results.remove(idx)
            } else {
                None
            }
        };

        if let Some(result) = maybe_result {
            for token in result.draft_tokens {
                if emitted >= max_new_tokens {
                    break;
                }
                let payload = serde_json::json!({ "token": token }).to_string();
                if socket.send(Message::Text(payload)).await.is_err() {
                    return;
                }
                emitted += 1;
            }
            continue;
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    if emitted == 0 {
        state.system_metrics.inc_verification_fallback();
        let fallback =
            generate_local_fallback_tokens(&state, prompt_context.as_str(), max_new_tokens).await;
        for token in fallback {
            let payload = serde_json::json!({ "token": token }).to_string();
            if socket.send(Message::Text(payload)).await.is_err() {
                return;
            }
        }
    }

    let _ = socket
        .send(Message::Text(
            serde_json::json!({"event": "done"}).to_string(),
        ))
        .await;
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatRequest {
    model: Option<String>,
    messages: Vec<ChatMessage>,
    stream: Option<bool>,
    max_tokens: Option<u32>,
    max_new_tokens: Option<u32>,
}

async fn chat_completions_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<ChatRequest>,
) -> impl IntoResponse {
    let stream_mode = req.stream.unwrap_or(false);
    let max_tokens = req.max_tokens.or(req.max_new_tokens).unwrap_or(256);

    let mut prompt = String::new();
    prompt.push_str("<|begin_of_text|>");
    for msg in &req.messages {
        prompt.push_str(&format!(
            "<|start_header_id|>{}\n\n{}<|eot_id|>",
            msg.role, msg.content
        ));
    }
    prompt.push_str("<|start_header_id|>assistant<|end_header_id|>\n\n");

    let request_id = format!("req-{}", now_ms());

    if stream_mode {
        let stream = async_stream::stream! {
            let mut engine_guard = state.engine.lock().await;
            if let Some(engine) = engine_guard.as_mut() {
                if let Ok(mut tokens) = engine.tokenize(&prompt, 4096) {
                    if !tokens.is_empty() && tokens[0] == 128000 {
                        tokens.remove(0);
                    }
                    if engine.eval(&tokens).is_ok() {
                        let mut emitted = 0;
                        while emitted < max_tokens {
                            if let Ok(logits) = engine.get_logits(128256) {
                                let mut best_idx = 0;
                                let mut best_val = -f32::INFINITY;
                                for (i, &val) in logits.iter().enumerate() {
                                    if val > best_val {
                                        best_val = val;
                                        best_idx = i;
                                    }
                                }

                                if best_idx == 128001 || best_idx == 128009 {
                                    break;
                                }

                                if let Ok(piece) = engine.token_to_piece(best_idx as i32) {
                                    let chunk = serde_json::json!({
                                        "id": request_id,
                                        "object": "chat.completion.chunk",
                                        "created": now_ms() / 1000,
                                        "model": req.model.as_deref().unwrap_or("shard-hybrid"),
                                        "choices": [{"index": 0, "delta": {"content": piece}, "finish_reason": serde_json::Value::Null}],
                                    });
                                    yield Ok::<_, std::convert::Infallible>(Event::default().data(chunk.to_string()));
                                }

                                if engine.eval(&[best_idx as i32]).is_err() {
                                    break;
                                }
                                emitted += 1;
                            } else {
                                break;
                            }
                        }
                    }
                }
            } else {
                let err = serde_json::json!({"error": "No model engine loaded in this daemon"});
                yield Ok(Event::default().data(err.to_string()));
            }

            let final_chunk = serde_json::json!({
                "id": request_id,
                "object": "chat.completion.chunk",
                "created": now_ms() / 1000,
                "model": req.model.as_deref().unwrap_or("shard-hybrid"),
                "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
            });
            yield Ok(Event::default().data(final_chunk.to_string()));
            yield Ok(Event::default().data("[DONE]"));
        };

        Sse::new(stream)
            .keep_alive(KeepAlive::default())
            .into_response()
    } else {
        // Run synchronously but hold the lock
        let mut full_text = String::new();
        {
            let mut engine_guard = state.engine.lock().await;
            if let Some(engine) = engine_guard.as_mut() {
                if let Ok(mut tokens) = engine.tokenize(&prompt, 4096) {
                    if !tokens.is_empty() && tokens[0] == 128000 {
                        tokens.remove(0);
                    }
                    if engine.eval(&tokens).is_ok() {
                        let mut emitted = 0;
                        while emitted < max_tokens {
                            if let Ok(logits) = engine.get_logits(128256) {
                                let mut best_idx = 0;
                                let mut best_val = -f32::INFINITY;
                                for (i, &val) in logits.iter().enumerate() {
                                    if val > best_val {
                                        best_val = val;
                                        best_idx = i;
                                    }
                                }

                                if best_idx == 128001 || best_idx == 128009 {
                                    break;
                                }

                                if let Ok(piece) = engine.token_to_piece(best_idx as i32) {
                                    full_text.push_str(&piece);
                                }

                                if engine.eval(&[best_idx as i32]).is_err() {
                                    break;
                                }
                                emitted += 1;
                            } else {
                                break;
                            }
                        }
                    }
                }
            } else {
                full_text = "No model engine loaded in this daemon".to_string();
            }
        }

        Json(serde_json::json!({
            "id": request_id,
            "object": "chat.completion",
            "created": now_ms() / 1000,
            "model": req.model.as_deref().unwrap_or("shard-hybrid"),
            "choices": [{"index": 0, "message": {"role": "assistant", "content": full_text}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0}
        })).into_response()
    }
}

async fn latency_profile_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let p = state.gossipsub_latency_hist.percentiles();
    Json(serde_json::json!({
        "source": "rust-sidecar",
        "gossipsub_propagation_ms": {
            "p50": p.p50_ms,
            "p90": p.p90_ms,
            "p99": p.p99_ms,
            "samples": p.samples,
        }
    }))
}

async fn metrics_handler(AxumState(state): AxumState<SharedState>) -> Response {
    let queue_depth = state.scout_work.lock().await.len();
    let active_node_count = state.node_metric_reports.lock().await.len();
    let node_latency_ms = state.avg_latency_ms.load(Ordering::Relaxed);
    let p = state.gossipsub_latency_hist.percentiles();
    let uptime_seconds = ((now_ms().saturating_sub(state.daemon_start)) / 1000) as u64;

    let body = state.system_metrics.render_prometheus(PrometheusSample {
        queue_depth,
        active_node_count,
        node_latency_ms,
        scheduler_decision_latency_ms: 0,
        e2e_latency_p50_ms: p.p50_ms,
        e2e_latency_p95_ms: p.p90_ms,
        e2e_latency_p99_ms: p.p99_ms,
        node_uptime_seconds: uptime_seconds,
    });

    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        body,
    )
        .into_response()
}

async fn scout_penalty_update_handler(
    AxumState(state): AxumState<SharedState>,
    Json(update): Json<ScoutPenaltyUpdate>,
) -> Json<serde_json::Value> {
    let mut penalties = state.scout_penalties.lock().await;
    let status = penalties.apply_update(update);
    Json(serde_json::json!({"ok": true, "status": status}))
}

async fn scout_penalty_status_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let penalties = state.scout_penalties.lock().await;
    Json(serde_json::json!({
        "ok": true,
        "peers": penalties.all_statuses(),
    }))
}

fn node_is_healthy(last_report_ms: u128, now: u128, timeout_ms: u128) -> bool {
    now.saturating_sub(last_report_ms) <= timeout_ms
}

async fn upsert_node_snapshot(
    state: &SharedState,
    node_pubkey: String,
    role: String,
    queue_depth: u64,
    node_latency_ms: u64,
    uptime_seconds: u64,
    reported_at_ms: u128,
) {
    let now = now_ms();
    let healthy = node_is_healthy(reported_at_ms, now, state.heartbeat_timeout_ms);
    let mut reports = state.node_metric_reports.lock().await;
    reports.insert(
        node_pubkey.clone(),
        NodeMetricSnapshot {
            node_pubkey,
            role,
            queue_depth,
            node_latency_ms,
            uptime_seconds,
            last_report_ms: reported_at_ms,
            healthy,
        },
    );
}

async fn persist_reputation_map(
    reputation_path: PathBuf,
    reputation: HashMap<String, NodeReputation>,
) {
    let _ = tokio::task::spawn_blocking(move || {
        save_reputation(&reputation_path, &reputation);
    })
    .await;
}

async fn mark_node_success(state: &SharedState, node_id: &str, latency_ms: f64) {
    let (path, snapshot) = {
        let mut rep = state.node_reputation.lock().await;
        let entry = rep
            .entry(node_id.to_string())
            .or_insert_with(|| NodeReputation {
                identity_score: 0.8,
                hardware_capability_score: 0.7,
                ..NodeReputation::default()
            });
        entry.update_success(latency_ms, now_ms());
        (state.node_reputation_path.clone(), rep.clone())
    };
    persist_reputation_map(path, snapshot).await;
}

async fn mark_node_failure(state: &SharedState, node_id: &str) {
    let (path, snapshot) = {
        let mut rep = state.node_reputation.lock().await;
        let entry = rep
            .entry(node_id.to_string())
            .or_insert_with(|| NodeReputation {
                identity_score: 0.8,
                hardware_capability_score: 0.7,
                ..NodeReputation::default()
            });
        entry.update_failure(now_ms());
        (state.node_reputation_path.clone(), rep.clone())
    };
    persist_reputation_map(path, snapshot).await;
}

async fn signed_register_node_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<SignedRequest<NodeRegistration>>,
) -> Json<serde_json::Value> {
    let signer = req.envelope.signer_pubkey_hex.clone();
    if let Err(detail) = req.envelope.verify() {
        state.system_metrics.inc_signature_verification_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": detail }));
    }

    if req.envelope.signer_pubkey_hex != req.envelope.payload.node_pubkey {
        state.system_metrics.inc_node_identity_auth_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({
            "ok": false,
            "detail": "signer pubkey does not match registration payload",
        }));
    }

    if !accept_replay_nonce(
        &state.replay_nonces,
        &req.envelope.signer_pubkey_hex,
        req.envelope.nonce,
    )
    .await
    {
        state.system_metrics.inc_node_identity_auth_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": "stale or replayed nonce" }));
    }

    let ts = req.envelope.payload.timestamp_ms.unwrap_or_else(now_ms);
    upsert_node_snapshot(
        &state,
        req.envelope.payload.node_pubkey,
        req.envelope.payload.role,
        0,
        0,
        0,
        ts,
    )
    .await;
    mark_node_success(&state, &signer, 0.0).await;

    Json(serde_json::json!({ "ok": true, "detail": "registered" }))
}

async fn signed_heartbeat_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<SignedRequest<NodeHeartbeat>>,
) -> Json<serde_json::Value> {
    let signer = req.envelope.signer_pubkey_hex.clone();
    if let Err(detail) = req.envelope.verify() {
        state.system_metrics.inc_signature_verification_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": detail }));
    }

    if req.envelope.signer_pubkey_hex != req.envelope.payload.node_pubkey {
        state.system_metrics.inc_node_identity_auth_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({
            "ok": false,
            "detail": "signer pubkey does not match heartbeat payload",
        }));
    }

    if !accept_replay_nonce(
        &state.replay_nonces,
        &req.envelope.signer_pubkey_hex,
        req.envelope.nonce,
    )
    .await
    {
        state.system_metrics.inc_node_identity_auth_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": "stale or replayed nonce" }));
    }

    let heartbeat = req.envelope.payload;
    let ts = heartbeat.timestamp_ms.unwrap_or_else(now_ms);
    upsert_node_snapshot(
        &state,
        heartbeat.node_pubkey,
        heartbeat.role,
        heartbeat.queue_depth,
        heartbeat.node_latency_ms,
        heartbeat.uptime_seconds,
        ts,
    )
    .await;
    mark_node_success(&state, &signer, heartbeat.node_latency_ms as f64).await;

    Json(serde_json::json!({ "ok": true, "detail": "heartbeat accepted" }))
}

async fn signed_metrics_report_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<SignedRequest<NodeMetricReport>>,
) -> Json<serde_json::Value> {
    let signer = req.envelope.signer_pubkey_hex.clone();
    if let Err(detail) = req.envelope.verify() {
        state.system_metrics.inc_signature_verification_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": detail }));
    }

    if req.envelope.signer_pubkey_hex != req.envelope.payload.node_pubkey {
        state.system_metrics.inc_node_identity_auth_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({
            "ok": false,
            "detail": "signer pubkey does not match metrics payload",
        }));
    }

    if !accept_replay_nonce(
        &state.replay_nonces,
        &req.envelope.signer_pubkey_hex,
        req.envelope.nonce,
    )
    .await
    {
        state.system_metrics.inc_node_identity_auth_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": "stale or replayed nonce" }));
    }

    let report = req.envelope.payload;
    let ts = report.timestamp_ms.unwrap_or_else(now_ms);

    upsert_node_snapshot(
        &state,
        report.node_pubkey.clone(),
        report.role.clone(),
        report.queue_depth,
        report.node_latency_ms,
        report.uptime_seconds,
        ts,
    )
    .await;

    let persist = PersistedNodeMetricReport {
        node_pubkey: report.node_pubkey,
        role: report.role,
        queue_depth: report.queue_depth,
        node_latency_ms: report.node_latency_ms,
        uptime_seconds: report.uptime_seconds,
        timestamp_ms: ts,
    };
    if let Err(e) = state.metrics_persistence.persist_report(&persist).await {
        tracing::warn!(%e, "failed to persist metric report");
    }
    mark_node_success(&state, &signer, report.node_latency_ms as f64).await;

    Json(serde_json::json!({ "ok": true, "detail": "metrics report accepted" }))
}

async fn signed_deregister_node_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<SignedRequest<NodeRegistration>>,
) -> Json<serde_json::Value> {
    let signer = req.envelope.signer_pubkey_hex.clone();
    if let Err(detail) = req.envelope.verify() {
        state.system_metrics.inc_signature_verification_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": detail }));
    }

    if req.envelope.signer_pubkey_hex != req.envelope.payload.node_pubkey {
        state.system_metrics.inc_node_identity_auth_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({
            "ok": false,
            "detail": "signer pubkey does not match deregistration payload",
        }));
    }

    if !accept_replay_nonce(
        &state.replay_nonces,
        &req.envelope.signer_pubkey_hex,
        req.envelope.nonce,
    )
    .await
    {
        state.system_metrics.inc_node_identity_auth_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": "stale or replayed nonce" }));
    }

    let mut reports = state.node_metric_reports.lock().await;
    reports.remove(req.envelope.payload.node_pubkey.as_str());
    Json(serde_json::json!({ "ok": true, "detail": "deregistered" }))
}

async fn metrics_summary_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let p = state.gossipsub_latency_hist.percentiles();
    let now = now_ms();
    let queue_depth = state.scout_work.lock().await.len();
    let mut reports = state.node_metric_reports.lock().await;
    let mut active_nodes = 0usize;
    let mut healthy_nodes = 0usize;
    let mut unhealthy_nodes = 0usize;

    for snapshot in reports.values_mut() {
        snapshot.healthy =
            node_is_healthy(snapshot.last_report_ms, now, state.heartbeat_timeout_ms);
        active_nodes += 1;
        if snapshot.healthy {
            healthy_nodes += 1;
        } else {
            unhealthy_nodes += 1;
        }
    }

    let counters = state.system_metrics.snapshot();
    let total_tokens = counters
        .tokens_processed_total
        .saturating_add(counters.tokens_offloaded_to_scouts_total);
    let offload_percentage = if total_tokens == 0 {
        0.0
    } else {
        (counters.tokens_offloaded_to_scouts_total as f64 / total_tokens as f64) * 100.0
    };
    let verification_rate = if counters.tokens_offloaded_to_scouts_total == 0 {
        100.0
    } else {
        ((counters.tokens_processed_total as f64
            / counters.tokens_offloaded_to_scouts_total as f64)
            * 100.0)
            .min(100.0)
    };
    let auth_failure_rate = if total_tokens == 0 {
        0.0
    } else {
        (counters.node_identity_auth_failures_total as f64 / total_tokens as f64) * 100.0
    };
    let cost = estimate_cost(&CostEstimateInput {
        tokens_processed_total: counters.tokens_processed_total,
        offload_percent: offload_percentage,
        gpu_utilization_delta_percent: (offload_percentage * 0.6).min(90.0),
        cloud_gpu_usd_per_million_tokens: 4.0,
    });

    Json(serde_json::json!({
        "active_nodes": active_nodes,
        "healthy_nodes": healthy_nodes,
        "unhealthy_nodes": unhealthy_nodes,
        "queue_depth": queue_depth,
        "node_identity_status": if unhealthy_nodes == 0 { "ok" } else { "degraded" },
        "average_latency_ms": state.avg_latency_ms.load(Ordering::Relaxed),
        "p95_latency_ms": p.p90_ms,
        "p99_latency_ms": p.p99_ms,
        "offload_percentage_estimate": offload_percentage,
        "verification_rate": verification_rate,
        "estimated_gpu_savings_percent": cost.estimated_savings_percent,
        "equivalent_cloud_gpu_cost_usd": cost.equivalent_cloud_gpu_cost_usd,
        "estimated_gpu_savings_usd": cost.estimated_savings_usd,
        "authentication_failure_rate": auth_failure_rate,
        "tokens_processed_total": counters.tokens_processed_total,
        "tokens_offloaded_to_scouts_total": counters.tokens_offloaded_to_scouts_total,
        "verification_fallback_total": counters.verification_fallback_total,
        "task_failures_total": counters.task_failures_total,
        "signature_verification_failures_total": counters.signature_verification_failures_total,
        "node_identity_auth_failures_total": counters.node_identity_auth_failures_total,
        "nodes": reports.values().cloned().collect::<Vec<NodeMetricSnapshot>>(),
    }))
}

async fn dashboard_handler() -> Html<&'static str> {
    Html(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Shard Operations Dashboard</title>
  <style>
    :root { --bg:#09121a; --card:#132534; --fg:#d9e9f7; --muted:#8fb2cf; --accent:#44d2ff; --ok:#4ae596; --warn:#ffbe55; }
    body { margin:0; font-family: "IBM Plex Sans", "Segoe UI", sans-serif; background: radial-gradient(circle at top, #163149, var(--bg)); color: var(--fg); }
    main { max-width: 1080px; margin: 0 auto; padding: 24px; }
    h1 { margin: 0 0 18px 0; font-size: 28px; letter-spacing: .02em; }
    .grid { display:grid; grid-template-columns: repeat(auto-fit, minmax(220px,1fr)); gap: 14px; }
    .card { background: linear-gradient(180deg, #173147, var(--card)); border:1px solid #295170; border-radius: 12px; padding: 14px; }
    .label { color: var(--muted); font-size: 12px; text-transform: uppercase; letter-spacing: .08em; }
    .value { font-size: 26px; margin-top: 4px; }
    table { width:100%; border-collapse: collapse; margin-top: 16px; }
    th, td { text-align:left; padding: 8px; border-bottom: 1px solid #26465f; }
    .ok { color: var(--ok); } .warn { color: var(--warn); }
  </style>
</head>
<body>
<main>
  <h1>Shard Operations Dashboard</h1>
  <div class="grid" id="cards"></div>
  <table>
    <thead><tr><th>Node</th><th>Role</th><th>Queue</th><th>Latency</th><th>Uptime</th><th>Status</th></tr></thead>
    <tbody id="nodes"></tbody>
  </table>
</main>
<script>
async function refresh() {
  const res = await fetch('/metrics/summary', { cache: 'no-store' });
  const data = await res.json();
  const cards = [
    ['Active nodes', data.active_nodes],
    ['Healthy nodes', data.healthy_nodes],
    ['Average latency', data.average_latency_ms + ' ms'],
    ['P95 latency', data.p95_latency_ms + ' ms'],
    ['Queue depth', data.queue_depth],
    ['Identity status', data.node_identity_status],
  ];
  document.getElementById('cards').innerHTML = cards.map(([k,v]) => `<div class="card"><div class="label">${k}</div><div class="value">${v}</div></div>`).join('');
  document.getElementById('nodes').innerHTML = (data.nodes || []).map(n => `<tr><td>${n.node_pubkey.slice(0,16)}...</td><td>${n.role}</td><td>${n.queue_depth}</td><td>${n.node_latency_ms} ms</td><td>${n.uptime_seconds}s</td><td class="${n.healthy ? 'ok' : 'warn'}">${n.healthy ? 'healthy' : 'unhealthy'}</td></tr>`).join('');
}
refresh(); setInterval(refresh, 5000);
</script>
</body></html>"#,
    )
}

fn create_router(state: SharedState) -> Router {
    let mut cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    if let Ok(raw_origins) = std::env::var("SHARD_CORS_ORIGINS") {
        let origins: Vec<HeaderValue> = raw_origins
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .filter_map(|value| HeaderValue::from_str(value).ok())
            .collect();
        if origins.is_empty() {
            cors = cors.allow_origin(Any);
        } else {
            cors = cors.allow_origin(origins);
        }
    } else {
        cors = cors.allow_origin(Any);
    }

    Router::new()
        .route("/health", get(health_handler))
        .route("/v1/system/health", get(health_handler))
        .route("/topology", get(topology_handler))
        .route("/v1/system/topology", get(topology_handler))
        .route("/wallet/address", get(wallet_address_handler))
        .route("/node/status", get(node_status_handler))
        .route("/node/ui", get(node_ui_handler))
        .route(
            "/node/toggle-participation",
            post(node_toggle_participation_handler),
        )
        .route("/node/logs", get(node_logs_handler))
        .route("/peers", get(peers_handler))
        .route("/v1/system/peers", get(peers_handler))
        .route("/ledger/head", get(ledger_head_handler))
        .route("/ledger/stats", get(ledger_stats_handler))
        .route("/ledger/export", get(ledger_export_handler))
        .route("/layers/next", get(next_layer_handler))
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
        .route("/submit-draft", post(submit_draft_handler))
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
        .route("/v1/chat/completions", post(chat_completions_handler))
        .route("/ws/generate", get(ws_generate_handler))
        .route("/scout/penalty", post(scout_penalty_update_handler))
        .route("/scout/penalty", get(scout_penalty_status_handler))
        .route("/metrics", get(metrics_handler))
        .route("/metrics/summary", get(metrics_summary_handler))
        .route("/metrics/latency-profile", get(latency_profile_handler))
        .route("/dashboard", get(dashboard_handler))
        .layer(cors)
        .with_state(state)
}

// ─── Main ───────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let mut cli = Cli::parse();
    let data = data_dir();
    tokio::fs::create_dir_all(&data).await?;
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

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&cli.log_level)),
        )
        .init();

    let topo_path = data.join("topology.json");
    let known_peers_path = data.join("known_peers.json");

    let file_bootstrap = if let Some(path) = &cli.bootstrap_file {
        read_bootstrap_file(path).await
    } else {
        Vec::new()
    };
    let persisted_bootstrap = load_persisted_peers(&known_peers_path).await;

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

    let bootstrap_addrs = unique_addrs(
        default_bootstrap
            .into_iter()
            .chain(cli.bootstrap.iter().cloned())
            .chain(file_bootstrap)
            .chain(persisted_bootstrap)
            .collect(),
    );

    // ── channels ──
    let (work_tx, mut work_rx) = mpsc::channel::<WorkRequest>(256);
    let (pipeline_tx, mut pipeline_rx) = mpsc::channel::<PipelineDispatch>(256);
    let (browser_result_tx, mut browser_result_rx) = mpsc::channel::<ForwardPassActivation>(256);

    let node_identity = NodeIdentity::load_or_create(&identity_path)?;
    let node_wallet = node_identity.wallet_address();
    let signing_key = node_identity.signing_key().clone();
    let id_keys = node_identity.libp2p_keypair()?;
    let local_peer_id = PeerId::from(id_keys.public());
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
            relay_server_enabled: cli.relay_server,
            contribute_enabled: cli.contribute,
            capacity: 100, // Default: 100 tokens/sec
            load: 0,
            latency_ms: 0.0,
        })),
        peers: Arc::new(Mutex::new(HashMap::new())),
        known_peers: Arc::new(Mutex::new(bootstrap_addrs.clone())),
        results: Arc::new(Mutex::new(VecDeque::new())),
        scout_work: Arc::new(Mutex::new(VecDeque::new())),
        work_tx,
        pipeline_tx,
        browser_result_tx,
        daemon_start: now_ms(),
        capacity: Arc::new(AtomicU32::new(100)), // Default: 100 tokens/sec
        current_load: Arc::new(AtomicU32::new(0)),
        avg_latency_ms: Arc::new(AtomicU32::new(0)),
        gossipsub_latency_hist: Arc::new(LatencyHistogram::new()),
        credit_nonce: Arc::new(AtomicU64::new(initial_credit_nonce)),
        scout_penalties: Arc::new(Mutex::new(ScoutPenaltyBook::default())),
        backward_passes: Arc::new(Mutex::new(VecDeque::new())),
        layer_routes: Arc::new(Mutex::new(LayerRoutingTable::default())),
        race_router: Arc::new(Mutex::new(RaceRouter::default())),
        ledger: Arc::new(Mutex::new(loaded_ledger)),
        ledger_store,
        browser_sessions: Arc::new(Mutex::new(HashMap::new())),
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
        heartbeat_interval_seconds: node_cfg.heartbeat_interval_seconds,
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
        let config = crate::inference::ShardInitConfig {
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
        match crate::inference::ShardEngine::load(&lib_path, &config) {
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

    // ── build swarm ──
    // ── build transport ──
    let tcp_config = libp2p::tcp::Config::default().nodelay(true);
    let dns_tcp = libp2p::dns::tokio::Transport::system(libp2p::tcp::tokio::Transport::new(
        tcp_config.clone(),
    ))?;
    let ws_dns_tcp = libp2p::websocket::Config::new(libp2p::dns::tokio::Transport::system(
        libp2p::tcp::tokio::Transport::new(tcp_config),
    )?);

    let tcp_ws = libp2p::core::transport::OrTransport::new(dns_tcp, ws_dns_tcp);

    let authenticated_transport = tcp_ws
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
        let gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(id_keys.clone()),
            gossipsub::Config::default(),
        )
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let kad = KadBehaviour::new(local_peer_id, MemoryStore::new(local_peer_id));
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
        let relay_server = relay::Behaviour::new(local_peer_id, Default::default());
        let dcutr = dcutr::Behaviour::new(local_peer_id);
        let autonat = autonat::v1::Behaviour::new(local_peer_id, autonat::v1::Config::default());
        let identify = identify::Behaviour::new(identify::Config::new(
            "/shard/1.0.0".to_string(),
            id_keys.public(),
        ));
        let ping = ping::Behaviour::new(ping::Config::new());
        ShardBehaviour {
            gossipsub,
            kad,
            handshake,
            verify,
            control_work,
            ledger_sync,
            relay_server,
            dcutr,
            autonat,
            identify,
            ping,
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
    let auction_topic = IdentTopic::new("auction.prompt");
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
    swarm.behaviour_mut().gossipsub.subscribe(&auction_topic)?;

    // ── listen addresses ──
    let tcp_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", cli.tcp_port).parse()?;
    let ws_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}/ws", cli.tcp_port + 100).parse()?;
    swarm.listen_on(tcp_addr)?;
    swarm.listen_on(ws_addr)?;
    let webrtc_addr: Multiaddr =
        format!("/ip4/0.0.0.0/udp/{}/webrtc-direct", cli.webrtc_port).parse()?;
    swarm.listen_on(webrtc_addr)?;
    let quic_addr: Multiaddr = format!("/ip4/0.0.0.0/udp/{}/quic-v1", cli.quic_port).parse()?;
    swarm.listen_on(quic_addr)?;

    // ── bootstrap peers ──
    for addr_str in &bootstrap_addrs {
        if let Ok(addr) = addr_str.parse::<Multiaddr>() {
            tracing::info!(%addr, "dialing bootstrap peer");
            let _ = swarm.dial(addr);
        }
    }

    // Advertise hosted layer start in Kademlia DHT provider index.
    let local_layer_key = provider_key(&cli.model_id, cli.layer_start);
    if let Err(e) = swarm.behaviour_mut().kad.start_providing(local_layer_key) {
        tracing::warn!(%e, "failed to publish local layer provider record");
    }

    telemetry_ws::spawn_telemetry_ws_server(state.clone(), cli.telemetry_ws_port);

    // ── spawn HTTP control-plane server ──
    let http_state = state.clone();
    let control_port = cli.control_port;
    tokio::spawn(async move {
        let app = create_router(http_state);
        let addr = SocketAddr::from(([0, 0, 0, 0], control_port));
        tracing::info!(%addr, "control-plane HTTP server starting");
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .expect("failed to bind control-plane port");
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
            tokio::time::sleep(Duration::from_secs(
                heartbeat_state.heartbeat_interval_seconds,
            ))
            .await;
            let heartbeat = SignedEnvelope::sign(
                NodeHeartbeat {
                    node_pubkey: pubkey.clone(),
                    role: role.clone(),
                    queue_depth: heartbeat_state.scout_work.lock().await.len() as u64,
                    node_latency_ms: heartbeat_state.avg_latency_ms.load(Ordering::Relaxed) as u64,
                    uptime_seconds: ((now_ms().saturating_sub(heartbeat_state.daemon_start)) / 1000)
                        as u64,
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
                payload.timestamp_ms.unwrap_or_else(now_ms),
            )
            .await;
        }
    });

    // Graceful signed deregistration signal handler.
    let shutdown_state = state.clone();
    let shutdown_signing_key = signing_key.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
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
            std::process::exit(0);
        }
    });

    println!();
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
        "  ║  Control API  : http://0.0.0.0:{}          ║",
        control_port
    );
    println!(
        "  ║  Telemetry WS : ws://0.0.0.0:{}/telemetry/ws ║",
        cli.telemetry_ws_port
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
        if cli.relay_server {
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
    let mut pending_handshakes: HashMap<OutboundRequestId, PeerId> = HashMap::new();
    let mut pending_layer_queries: HashMap<libp2p::kad::QueryId, (String, u32)> = HashMap::new();
    let mut pending_ledger_sync: HashMap<OutboundRequestId, (PeerId, u64)> = HashMap::new();
    let layer_ttl_ms: u128 = 60_000;
    let mut next_layer_announcement_ms = 0u128;
    let mut next_ledger_snapshot_ms = 0u128;

    // ── main event loop ──
    loop {
        tokio::select! {
            _ = reconnect_tick.tick() => {
                let known = state.known_peers.lock().await.clone();
                let connected: HashSet<String> = state.peers.lock().await.keys().cloned().collect();
                for addr_str in known {
                    if let Ok(addr) = addr_str.parse::<Multiaddr>() {
                        let is_self = addr.to_string().contains(&local_peer_id.to_string());
                        if !is_self {
                            // Attempt periodic redial for resilience.
                            if let Err(err) = swarm.dial(addr.clone()) {
                                tracing::debug!(%addr, %err, "reconnect dial skipped/failed");
                            } else {
                                tracing::debug!(%addr, connected = connected.len(), "reconnect dial attempted");
                            }
                        }
                    }
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
                match serde_json::to_vec(&work_req) {
                    Ok(payload) => {
                        match swarm.behaviour_mut().gossipsub.publish(work_topic.clone(), payload) {
                            Ok(_) => tracing::info!(id = %work_req.request_id, "published WorkRequest to gossipsub"),
                            Err(e) => tracing::warn!(id = %work_req.request_id, %e, "gossipsub publish failed (no peers?)"),
                        }
                    }
                    Err(e) => tracing::error!(%e, "failed to serialize WorkRequest"),
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
                            if let Ok(result) = serde_json::from_slice::<WorkResponse>(&message.data) {
                                let peer_is_blackholed = {
                                    let mut penalties = state.scout_penalties.lock().await;
                                    penalties.is_blackholed(&result.peer_id)
                                };
                                if peer_is_blackholed {
                                    tracing::warn!(peer = %result.peer_id, "dropping WorkResponse from blackholed scout peer");
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

                                let mut q = state.results.lock().await;
                                q.push_back(result);
                                while q.len() > 128 { q.pop_front(); }
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
                            "work request via req/resp -> publishing to gossipsub"
                        );
                        if let Ok(payload) = serde_json::to_vec(&request) {
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
                        tracing::debug!(?event, "verify protocol event");
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

                    // Note: relay client disabled - libp2p API changed
                    // SwarmEvent::Behaviour(ShardBehaviourEvent::RelayClient(event)) => { ... }

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
                    }

                    // ── identify ──
                    SwarmEvent::Behaviour(ShardBehaviourEvent::Identify(event)) => {
                        match event {
                            identify::Event::Received { peer_id, info, .. } => {
                                tracing::info!(%peer_id, protocol_version = %info.protocol_version, "identify info received");
                                let observed_addr = info.observed_addr;
                                tracing::info!(%peer_id, ?observed_addr, "observed address");
                                let mut topo = state.topology.lock().await;
                                // Update with observed public address if behind NAT
                                if topo.public_api_addr.is_none() && !observed_addr.to_string().starts_with("/ip4/127.0.0.1") && !observed_addr.to_string().starts_with("/ip6/::1") {
                                    topo.public_api_addr = Some(format!("{}/p2p/{}", observed_addr, local_peer_id));
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

                        tracing::info!(%peer_id, ?endpoint, "peer connected");
                        let remote_addr = endpoint.get_remote_address().to_string();

                        {
                            let mut peers = state.peers.lock().await;
                            peers.insert(
                                peer_id.to_string(),
                                PeerInfo {
                                    peer_id: peer_id.to_string(),
                                    connected_at: now_ms(),
                                    last_seen_at: now_ms(),
                                    addrs: vec![remote_addr.clone()],
                                    verified: false,
                                    handshake_failures: 0,
                                },
                            );
                        }

                        {
                            let mut known = state.known_peers.lock().await;
                            known.push(remote_addr);
                            *known = unique_addrs(known.clone());
                            save_persisted_peers(&known_peers_path, &known).await;
                        }

                        let req = Heartbeat { kind: "PING".into(), sent_at_ms: now_ms() };
                        let id = swarm.behaviour_mut().handshake.send_request(&peer_id, req);
                        pending_handshakes.insert(id, peer_id);
                    }

                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        tracing::info!(%peer_id, "peer disconnected");
                        let mut peers = state.peers.lock().await;
                        peers.remove(&peer_id.to_string());
                    }

                    SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                        tracing::warn!(?peer_id, %error, "outgoing connection error");
                    }

                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        accept_replay_nonce, node_is_healthy, should_reject_peer_connection, unique_addrs,
        validate_work_request, LatencyHistogram, ScoutPenaltyBook, ScoutPenaltyUpdate, WorkRequest,
    };
    use std::collections::HashMap;
    use std::sync::Arc;
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
    fn work_request_validation_enforces_bounds() {
        let ok = WorkRequest {
            request_id: "abc".into(),
            prompt_context: "hello".into(),
            min_tokens: 4,
            created_at_ms: None,
        };
        assert!(validate_work_request(&ok).is_ok());

        let bad = WorkRequest {
            request_id: "".into(),
            prompt_context: "hello".into(),
            min_tokens: 0,
            created_at_ms: None,
        };
        assert!(validate_work_request(&bad).is_err());
    }
    #[test]
    fn test_malicious_scout_blacklist_trigger() {
        let mut penalties = ScoutPenaltyBook::default();
        let peer_id = "PeerID_C".to_string();

        let mut status = penalties.apply_update(ScoutPenaltyUpdate {
            peer_id: peer_id.clone(),
            accepted: true,
            probability_bound: 1.0e-16,
            reason: None,
        });
        assert!(!status.blackholed);

        for _ in 0..5 {
            status = penalties.apply_update(ScoutPenaltyUpdate {
                peer_id: peer_id.clone(),
                accepted: false,
                probability_bound: 1.0e-12,
                reason: Some("poisoned draft".to_string()),
            });
        }

        assert!(status.score < 55);
        assert!(status.blackholed);
        assert!(penalties.is_blackholed(&peer_id));
    }

    #[test]
    fn test_honest_scout_baseline_reputation() {
        let mut penalties = ScoutPenaltyBook::default();
        let peer_id = "PeerID_A".to_string();
        let mut status = penalties.apply_update(ScoutPenaltyUpdate {
            peer_id: peer_id.clone(),
            accepted: true,
            probability_bound: 1.0e-16,
            reason: None,
        });

        for _ in 0..9 {
            status = penalties.apply_update(ScoutPenaltyUpdate {
                peer_id: peer_id.clone(),
                accepted: true,
                probability_bound: 1.0e-16,
                reason: None,
            });
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

        let mut status = penalties.apply_update(ScoutPenaltyUpdate {
            peer_id: peer_id.clone(),
            accepted: true,
            probability_bound: 1.0e-16,
            reason: None,
        });

        // Mixed quality scout; keep recent sliding success ratio above ban threshold.
        for accepted in [true, false, true, false, true, true, false, true, false] {
            status = penalties.apply_update(ScoutPenaltyUpdate {
                peer_id: peer_id.clone(),
                accepted,
                probability_bound: if accepted { 1.0e-16 } else { 1.0e-6 },
                reason: if accepted {
                    None
                } else {
                    Some("invalid draft".to_string())
                },
            });
        }

        assert!(status.score >= 55);
        assert!(!status.blackholed);
    }

    #[test]
    fn test_blacklist_enforcement_rejects_connection() {
        let mut penalties = ScoutPenaltyBook::default();
        let peer_id = "PeerID_C".to_string();

        penalties.apply_update(ScoutPenaltyUpdate {
            peer_id: peer_id.clone(),
            accepted: true,
            probability_bound: 1.0e-16,
            reason: None,
        });
        for _ in 0..5 {
            penalties.apply_update(ScoutPenaltyUpdate {
                peer_id: peer_id.clone(),
                accepted: false,
                probability_bound: 1.0e-12,
                reason: Some("poisoned".to_string()),
            });
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

    #[test]
    fn node_health_timeout_marks_stale_unhealthy() {
        assert!(node_is_healthy(10_000, 15_000, 5_000));
        assert!(!node_is_healthy(10_000, 20_001, 5_000));
    }
}
