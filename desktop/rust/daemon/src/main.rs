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

#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::unnecessary_cast)]

use anyhow::Result;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::{
    extract::Path as AxumPath,
    extract::Query,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State as AxumState,
    },
    http::{HeaderMap, HeaderValue, Method, StatusCode},
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
use shard_common::types::{WorkRequest, WorkResponse};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    hash::{Hash, Hasher},
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

pub mod api;
pub mod bootstrap_discovery;
pub mod scheduler;
pub mod telemetry_ws;
use api::*;
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
// use shard_gateway::rate_limiter::{RateLimitConfig, RateLimitStatus, RateLimiter};
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CanaryRolloutConfig {
    enabled: bool,
    canary_model_id: String,
    traffic_percent: u8,
    max_avg_latency_ms: u64,
    min_acceptance_rate: f64,
    max_reject_rate: f64,
    min_samples: u64,
}

impl CanaryRolloutConfig {
    fn from_env(default_model_id: &str) -> Self {
        let enabled = std::env::var("SHARD_CANARY_ENABLED")
            .ok()
            .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
            .unwrap_or(false);
        let canary_model_id = std::env::var("SHARD_CANARY_MODEL_ID")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "verifier-v2".to_string());
        let traffic_percent = std::env::var("SHARD_CANARY_TRAFFIC_PERCENT")
            .ok()
            .and_then(|v| v.parse::<u8>().ok())
            .map(|v| v.min(100))
            .unwrap_or(10);
        let max_avg_latency_ms = std::env::var("SHARD_CANARY_MAX_AVG_LATENCY_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(2500);
        let min_acceptance_rate = std::env::var("SHARD_CANARY_MIN_ACCEPTANCE_RATE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .map(|v| v.clamp(0.0, 1.0))
            .unwrap_or(0.60);
        let max_reject_rate = std::env::var("SHARD_CANARY_MAX_REJECT_RATE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .map(|v| v.clamp(0.0, 1.0))
            .unwrap_or(0.40);
        let min_samples = std::env::var("SHARD_CANARY_MIN_SAMPLES")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(20);

        Self {
            enabled,
            canary_model_id,
            traffic_percent,
            max_avg_latency_ms,
            min_acceptance_rate,
            max_reject_rate,
            min_samples,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CanaryRolloutStatus {
    rollback_active: bool,
    rollback_reason: Option<String>,
    last_evaluated_ms: u128,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub(crate) struct CanaryRolloutStats {
    canary_requests_total: u64,
    canary_latency_sum_ms: u64,
    canary_acceptance_samples: u64,
    canary_acceptance_sum: f64,
    canary_reject_sum: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct CanaryRolloutController {
    config: CanaryRolloutConfig,
    status: CanaryRolloutStatus,
    stats: CanaryRolloutStats,
    stable_model_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CanaryDecision {
    pub(crate) use_canary: bool,
    pub(crate) selected_model_id: String,
    pub(crate) canary_eligible: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ModelRolloutSnapshot {
    stable_model_id: String,
    config: CanaryRolloutConfig,
    status: CanaryRolloutStatus,
    stats: CanaryRolloutStats,
}

impl CanaryRolloutController {
    fn new(stable_model_id: String, config: CanaryRolloutConfig) -> Self {
        Self {
            config,
            status: CanaryRolloutStatus {
                rollback_active: false,
                rollback_reason: None,
                last_evaluated_ms: 0,
            },
            stats: CanaryRolloutStats::default(),
            stable_model_id,
        }
    }

    pub(crate) fn decide(&self, request_id: &str, canary_eligible: bool) -> CanaryDecision {
        if !self.config.enabled
            || self.status.rollback_active
            || !canary_eligible
            || self.config.traffic_percent == 0
        {
            return CanaryDecision {
                use_canary: false,
                selected_model_id: self.stable_model_id.clone(),
                canary_eligible,
            };
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        request_id.hash(&mut hasher);
        let bucket = (hasher.finish() % 100) as u8;
        let use_canary = bucket < self.config.traffic_percent;
        CanaryDecision {
            use_canary,
            selected_model_id: if use_canary {
                self.config.canary_model_id.clone()
            } else {
                self.stable_model_id.clone()
            },
            canary_eligible,
        }
    }

    pub(crate) fn canary_model_id(&self) -> &str {
        self.config.canary_model_id.as_str()
    }

    pub(crate) fn snapshot(&self) -> ModelRolloutSnapshot {
        ModelRolloutSnapshot {
            stable_model_id: self.stable_model_id.clone(),
            config: self.config.clone(),
            status: self.status.clone(),
            stats: self.stats.clone(),
        }
    }

    pub(crate) fn record_request_outcome(
        &mut self,
        decision: &CanaryDecision,
        latency_ms: u64,
        acceptance_rate: Option<f64>,
        reject_rate: Option<f64>,
    ) {
        if !decision.use_canary {
            return;
        }
        self.stats.canary_requests_total = self.stats.canary_requests_total.saturating_add(1);
        self.stats.canary_latency_sum_ms =
            self.stats.canary_latency_sum_ms.saturating_add(latency_ms);
        if let (Some(acc), Some(rej)) = (acceptance_rate, reject_rate) {
            self.stats.canary_acceptance_samples =
                self.stats.canary_acceptance_samples.saturating_add(1);
            self.stats.canary_acceptance_sum += acc;
            self.stats.canary_reject_sum += rej;
        }
        self.evaluate_auto_rollback();
    }

    fn evaluate_auto_rollback(&mut self) {
        self.status.last_evaluated_ms = now_ms();
        if self.status.rollback_active {
            return;
        }
        if self.stats.canary_requests_total < self.config.min_samples {
            return;
        }
        let avg_latency = if self.stats.canary_requests_total == 0 {
            0.0
        } else {
            self.stats.canary_latency_sum_ms as f64 / self.stats.canary_requests_total as f64
        };
        if avg_latency > self.config.max_avg_latency_ms as f64 {
            self.status.rollback_active = true;
            self.status.rollback_reason = Some(format!(
                "canary avg latency {:.1}ms exceeded threshold {}ms",
                avg_latency, self.config.max_avg_latency_ms
            ));
            return;
        }

        if self.stats.canary_acceptance_samples >= self.config.min_samples {
            let acceptance_rate =
                self.stats.canary_acceptance_sum / self.stats.canary_acceptance_samples as f64;
            let reject_rate =
                self.stats.canary_reject_sum / self.stats.canary_acceptance_samples as f64;
            if acceptance_rate < self.config.min_acceptance_rate {
                self.status.rollback_active = true;
                self.status.rollback_reason = Some(format!(
                    "canary acceptance rate {:.3} below threshold {:.3}",
                    acceptance_rate, self.config.min_acceptance_rate
                ));
                return;
            }
            if reject_rate > self.config.max_reject_rate {
                self.status.rollback_active = true;
                self.status.rollback_reason = Some(format!(
                    "canary reject rate {:.3} above threshold {:.3}",
                    reject_rate, self.config.max_reject_rate
                ));
            }
        }
    }

    pub(crate) fn reset_rollback(&mut self) {
        self.status.rollback_active = false;
        self.status.rollback_reason = None;
        self.status.last_evaluated_ms = now_ms();
        self.stats = CanaryRolloutStats::default();
    }
}

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
    updated_at_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedBootstrapRegistry {
    entries: Vec<BootstrapRegistryEntry>,
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
    /// First time this peer was seen (for stability tracking)
    first_seen_at: u128,
    /// Number of successful handshakes
    successful_handshakes: u32,
    /// Average latency in ms
    avg_latency_ms: f32,
    /// Number of consecutive connection failures (for bootstrap removal)
    pub connection_failures: u32,
}

#[derive(Clone)]
pub(crate) struct SharedState {
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
    scout_draft_rx: Arc<Mutex<Option<mpsc::Receiver<ScoutDraft>>>>,
    /// Channel for announcing bans to the network
    ban_tx: mpsc::Sender<(String, String)>,
    /// Timeout tracker for speculative decoding
    scout_timeout_tracker: Arc<Mutex<ScoutTimeoutTracker>>,
    /// Bootstrap peer failure tracking (peer_id -> consecutive failures)
    /// Used to remove unreachable bootstraps after MAX_BOOTSTRAP_FAILURES
    bootstrap_failures: Arc<Mutex<HashMap<String, u32>>>,
    bootstrap_registry: Arc<Mutex<HashMap<String, BootstrapRegistryEntry>>>,
    bootstrap_registry_path: PathBuf,
    scheduler_decisions: Arc<Mutex<VecDeque<SchedulerDecisionLog>>>,
    canary_rollout: Arc<Mutex<CanaryRolloutController>>,
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
    /// Timeout in ms to wait for scout draft (default: 800ms).
    scout_timeout_ms: u64,
    /// Cooldown in ms after 3 consecutive timeouts (default: 60000ms).
    scout_cooldown_ms: u64,
    /// Number of consecutive timeouts before cooldown.
    max_consecutive_timeouts: u32,
    /// Number of draft tokens to request from scout.
    draft_token_count: usize,
}

impl Default for SpeculativeConfig {
    fn default() -> Self {
        Self {
            scout_timeout_ms: std::env::var("SHARD_SCOUT_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                // Browser scouts poll work over HTTP; 400ms is too short for
                // realistic poll + draft + submit round-trips.
                .unwrap_or(8000),
            scout_cooldown_ms: 60000,
            max_consecutive_timeouts: 3,
            draft_token_count: 4,
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

/// Result of verifying draft tokens against the verifier model.
#[derive(Clone, Debug)]
struct DraftVerificationResult {
    accepted_tokens: Vec<i32>,
    accepted_text: String,
    first_rejection_idx: Option<usize>,
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

        if !blackholed && global_p95 > 0 && entry.latency_ms > (global_p95 as f64 * 1.5) as u64 {
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
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("shard")
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
    addr.parse::<Multiaddr>()
        .ok()
        .and_then(|multiaddr| extract_peer_id_from_multiaddr(&multiaddr).map(|peer| peer.to_string()))
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
        "disabled" | "disable" | "off" | "false" | "none" => {
            HardcodedBootstrapMode::Disabled
        }
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
            if !allow_private && is_non_public_bootstrap_addr(&multiaddr) {
                tracing::warn!(%addr, "dropping non-public bootstrap address; set SHARD_ALLOW_PRIVATE_BOOTSTRAP=true to allow");
                return false;
            }
            true
        })
        .collect()
}

fn record_bootstrap_failure(
    known: &mut Vec<String>,
    failures: &mut HashMap<String, u32>,
    peer_id: &PeerId,
) -> bool {
    let peer_id_str = peer_id.to_string();
    let is_bootstrap = known.iter().any(|addr| {
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
        known.retain(|addr| {
            if let Ok(multiaddr) = addr.parse::<libp2p::Multiaddr>() {
                extract_peer_id_from_multiaddr(&multiaddr) != Some(*peer_id)
            } else {
                true
            }
        });
        failures.remove(&peer_id_str);
        return true;
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
        .map(|entry| (entry.peer_id.clone(), entry))
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
        .route("/connectivity", get(connectivity_handler))
        .route("/v1/system/connectivity", get(connectivity_handler))
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

    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
    let filter_layer = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&cli.log_level));

    let fmt_layer = tracing_subscriber::fmt::layer().json();

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(opentelemetry_otlp::new_exporter().tonic())
        .with_trace_config(opentelemetry_sdk::trace::config().with_resource(
            opentelemetry_sdk::Resource::new(vec![opentelemetry::KeyValue::new(
                "service.name",
                "shard-daemon",
            )]),
        ))
        .install_batch(opentelemetry_sdk::runtime::Tokio)
        .expect("Failed to initialize OpenTelemetry");

    let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(telemetry_layer)
        .try_init()
        .expect("Failed to initialize tracing registry");

    let topo_path = data.join("topology.json");
    let known_peers_path = data.join("known_peers.json");
    let bootstrap_registry_path = data.join("bootstrap_registry.json");

    let file_bootstrap = if let Some(path) = &cli.bootstrap_file {
        read_bootstrap_file(path).await
    } else {
        Vec::new()
    };
    let persisted = load_persisted_peers(&known_peers_path).await;
    let loaded_bootstrap_registry = load_bootstrap_registry(&bootstrap_registry_path).await;

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
        "/ip4/35.175.242.222/tcp/4001/p2p/12D3KooWConhJakwyGN72uZ1Jtxi3LFecN3cYKxEX3aNLDAo48by"
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
            Err(e) => {
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

    let mut bootstrap_sources = default_bootstrap;
    bootstrap_sources.extend(cli.bootstrap_node.clone());
    bootstrap_sources.extend(file_bootstrap);
    bootstrap_sources.extend(persisted);
    bootstrap_sources.extend(url_bootstrap);

    let hardcoded_mode = parse_hardcoded_bootstrap_mode(
        std::env::var("SHARD_HARDCODED_BOOTSTRAP_MODE").ok(),
    );
    let include_hardcoded =
        should_include_hardcoded_bootstrap(hardcoded_mode, !bootstrap_sources.is_empty());
    if include_hardcoded {
        bootstrap_sources.extend(hardcoded_bootstrap);
    }

    let bootstrap_addrs = filter_bootstrap_addrs(bootstrap_sources, allow_private_bootstrap);
    tracing::info!(
        bootstrap_count = bootstrap_addrs.len(),
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
    let canary_rollout_cfg = CanaryRolloutConfig::from_env(cli.model_id.as_str());

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
        ban_tx,
        scout_timeout_tracker: Arc::new(Mutex::new(ScoutTimeoutTracker::new())),
        bootstrap_failures: Arc::new(Mutex::new(HashMap::new())),
        bootstrap_registry: Arc::new(Mutex::new(loaded_bootstrap_registry)),
        bootstrap_registry_path,
        scheduler_decisions: Arc::new(Mutex::new(VecDeque::new())),
        canary_rollout: Arc::new(Mutex::new(CanaryRolloutController::new(
            cli.model_id.clone(),
            canary_rollout_cfg,
        ))),
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
        let gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(id_keys.clone()),
            gossipsub::Config::default(),
        )
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let mut kad = KadBehaviour::new(local_peer_id, MemoryStore::new(local_peer_id));
        if !cli.relay_mode {
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
        let autonat = autonat::v1::Behaviour::new(local_peer_id, autonat::v1::Config::default());
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
    let auction_topic = IdentTopic::new("auction.prompt");
    let ban_topic = IdentTopic::new("shard-ban-list");

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
    swarm.behaviour_mut().gossipsub.subscribe(&ban_topic)?;

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

    telemetry_ws::spawn_telemetry_ws_server(state.clone(), cli.telemetry_ws_port);

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
            let client = reqwest::Client::new();
            loop {
                tokio::time::sleep(Duration::from_secs(300)).await; // Check every 5 minutes

                let now = now_ms();
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
                drop(topo);

                if let Some(multiaddr) = addrs.first() {
                    let registration = bootstrap_discovery::BootstrapRegistration {
                        peer_id: local_peer_id.clone(),
                        multiaddr: multiaddr.clone(),
                        stability_score: 100, // TODO: calculate dynamically
                        uptime_hours: uptime_hours_u64,
                        version: env!("CARGO_PKG_VERSION").to_string(),
                    };

                    if let Err(e) =
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
                        if should_attempt_reconnect(&addr, &local_peer_id, &connected) {
                            if let Err(err) = swarm.dial(addr.clone()) {
                                let peer_ref = peer_id_from_addr_str(&addr_str)
                                    .unwrap_or_else(|| "<unknown>".to_string());
                                tracing::debug!(peer_id = %peer_ref, %err, "reconnect dial skipped/failed");
                            } else {
                                let peer_ref = peer_id_from_addr_str(&addr_str)
                                    .unwrap_or_else(|| "<unknown>".to_string());
                                tracing::info!(peer_id = %peer_ref, "reconnect dial attempted for disconnected peer");
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

                                    // If we are acting as a Scout, pick up this work
                                    let mut queue = state.scout_work.lock().await;
                                    queue.push_back(envelope.payload);
                                    while queue.len() > 1024 {
                                        queue.pop_front();
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

                    SwarmEvent::Behaviour(ShardBehaviourEvent::RelayClient(event)) => {
                        tracing::info!("RelayClient event: {:?}", event);
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
                            }
                            autonat::NatStatus::Private => {
                                topo.is_public = false;
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
                                // Update with observed public address if behind NAT
                                if topo.public_api_addr.is_none() && !observed_addr.to_string().starts_with("/ip4/127.0.0.1") && !observed_addr.to_string().starts_with("/ip6/::1") {
                                    topo.public_api_addr = Some(format!("{}/p2p/{}", observed_addr, local_peer_id));
                                }

                                let mut learned_addrs = Vec::new();
                                for listen_addr in &info.listen_addrs {
                                    let mut full_addr = listen_addr.clone();
                                    if !full_addr.iter().any(|p| matches!(p, libp2p::multiaddr::Protocol::P2p(_))) {
                                        full_addr = full_addr.with(libp2p::multiaddr::Protocol::P2p(peer_id));
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
                                    first_seen_at: now_ms(),
                                    successful_handshakes: 0,
                                    avg_latency_ms: 0.0,
                                    connection_failures: 0,
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
                        tracing::warn!(?peer_id, "outgoing connection error");
                        tracing::debug!(?peer_id, %error, "outgoing connection error details");

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
}

#[cfg(test)]
mod tests {
    use super::{
        accept_replay_nonce, filter_bootstrap_addrs, is_non_public_bootstrap_addr,
        load_bootstrap_registry, node_is_healthy, parse_hardcoded_bootstrap_mode,
        peer_id_from_addr_str, record_bootstrap_failure, save_bootstrap_registry,
        should_attempt_reconnect, should_include_hardcoded_bootstrap,
        should_reject_peer_connection, unique_addrs, validate_work_request,
        BootstrapRegistryEntry, CanaryRolloutConfig, CanaryRolloutController,
        HardcodedBootstrapMode, LatencyHistogram, ScoutPenaltyBook, ScoutPenaltyUpdate,
        ScoutTimeoutTracker, SpeculativeConfig, WorkRequest, MAX_BOOTSTRAP_FAILURES,
    };
    use libp2p::{Multiaddr, PeerId};
    use std::collections::{HashMap, HashSet};
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
        let input = vec![
            "/ip4/192.168.1.85/tcp/4001".to_string(),
            "/ip4/35.175.242.222/tcp/4001".to_string(),
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

        for _ in 0..5 {
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
        for _ in 0..5 {
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
    fn bootstrap_failures_remove_unhealthy_bootstrap_peer() {
        let peer = PeerId::random();
        let addr = format!("/ip4/127.0.0.1/tcp/4001/p2p/{peer}");
        let mut known = vec![addr];
        let mut failures = HashMap::new();
        let mut removed = false;

        for _ in 0..MAX_BOOTSTRAP_FAILURES {
            removed = record_bootstrap_failure(&mut known, &mut failures, &peer);
        }

        assert!(removed);
        assert!(known.is_empty());
        assert!(!failures.contains_key(&peer.to_string()));
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
        let controller = CanaryRolloutController::new("default-model".to_string(), cfg);
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
        let mut controller = CanaryRolloutController::new("default-model".to_string(), cfg);
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
                version: "0.6.1".to_string(),
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
}
