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
use axum::{
    extract::Path as AxumPath,
    extract::Query,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State as AxumState,
    },
    http::{HeaderValue, Method},
    response::IntoResponse,
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
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, Mutex};
use tower_http::cors::{Any, CorsLayer};

mod crypto;
mod ledger;
mod mesh;
mod network;
mod telemetry_ws;
use crypto::identity::NodeIdentity;
use crypto::wallet_backup::{export_wallet, import_wallet, verify_backup};
use ledger::state::{ComputeCreditTx, LedgerState};
use mesh::race_router::{RaceKey, RaceRouter, RaceSubmitOutcome};
use network::layer_registry::{provider_key, LayerHostAnnouncement, LayerRoutingTable};
use network::obfuscation::{deobfuscate_bytes, obfuscate_bytes, random_nonce};
use network::tensor_wire::TensorWirePacket;

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
    browser_sessions: Arc<Mutex<HashMap<String, BrowserLayerSession>>>,
    browser_work: Arc<Mutex<VecDeque<BrowserLayerWorkItem>>>,
    node_wallet: String,
    model_id: String,
    layer_start: u32,
    layer_end: u32,
    race_pool_size: usize,
    race_timeout_ms: u64,
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

fn validate_work_request(req: &WorkRequest) -> Result<(), String> {
    if req.request_id.trim().is_empty() || req.request_id.len() > 128 {
        return Err("request_id must be non-empty and <= 128 chars".into());
    }
    if req.prompt_context.trim().is_empty() {
        return Err("prompt_context must be non-empty".into());
    }
    if req.prompt_context.len() > 16000 {
        return Err("prompt_context exceeds 16000 chars".into());
    }
    if req.min_tokens <= 0 || req.min_tokens > 512 {
        return Err("min_tokens must be between 1 and 512".into());
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

async fn topology_handler(AxumState(state): AxumState<SharedState>) -> Json<serde_json::Value> {
    let topo = state.topology.lock().await;
    let known = state.known_peers.lock().await;
    let capacity = state.capacity.load(Ordering::Relaxed);
    let load = state.current_load.load(Ordering::Relaxed);
    let latency_ms = state.avg_latency_ms.load(Ordering::Relaxed);
    Json(serde_json::json!({
        "status": "ok",
        "source": "rust-sidecar",
        "shard_peer_id": topo.local_peer_id,
        "shard_webrtc_multiaddr": topo.webrtc_addr,
        "shard_quic_multiaddr": topo.quic_addr,
        "shard_ws_multiaddr": topo.ws_addr,
        "listen_addrs": topo.listen_addrs,
        "known_peer_count": known.len(),
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
    Json(serde_json::json!({
        "ok": true,
        "model_id": model_id,
        "current_layer": query.current_layer,
        "next_layer": query.current_layer.saturating_add(1),
        "peers": peers,
        "count": peers.len(),
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
    if let Err(detail) = validate_work_request(&req) {
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
        Err(e) => Json(serde_json::json!({ "ok": false, "detail": format!("channel error: {e}") })),
    }
}

async fn pop_result_handler(
    AxumState(state): AxumState<SharedState>,
    Query(query): Query<PopResultQuery>,
) -> Json<serde_json::Value> {
    let mut results = state.results.lock().await;
    if let Some(request_id) = query.request_id {
        if let Some(idx) = results.iter().position(|r| r.request_id == request_id) {
            if let Some(result) = results.remove(idx) {
                return Json(serde_json::json!({ "result": result }));
            }
        }
        return Json(serde_json::json!({ "result": null }));
    }

    match results.pop_front() {
        Some(result) => Json(serde_json::json!({ "result": result })),
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
    if submission.work_id.trim().is_empty() || submission.scout_id.trim().is_empty() {
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

    let mut results = state.results.lock().await;
    results.push_back(response);
    while results.len() > 2048 {
        results.pop_front();
    }

    Json(serde_json::json!({ "ok": true, "detail": "draft queued" }))
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
        prompt_context,
        min_tokens: 1,
        created_at_ms: Some(now_ms()),
    };

    if let Err(detail) = validate_work_request(&work) {
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

    let _ = socket
        .send(Message::Text(
            serde_json::json!({"event": "done"}).to_string(),
        ))
        .await;
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
        .route("/topology", get(topology_handler))
        .route("/wallet/address", get(wallet_address_handler))
        .route("/peers", get(peers_handler))
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
        .route("/pop-result", get(pop_result_handler))
        .route("/pop-work", get(pop_work_handler))
        .route("/submit-draft", post(submit_draft_handler))
        .route("/ws/generate", get(ws_generate_handler))
        .route("/scout/penalty", post(scout_penalty_update_handler))
        .route("/scout/penalty", get(scout_penalty_status_handler))
        .route("/metrics/latency-profile", get(latency_profile_handler))
        .layer(cors)
        .with_state(state)
}

// ─── Main ───────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let mut cli = Cli::parse();
    let data = data_dir();
    tokio::fs::create_dir_all(&data).await?;
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

    // Default bootstrap peers - public Shard nodes that are always online
    // New nodes will automatically connect to the network via these
    let default_bootstrap = vec![
        // Public bootstrap Shard (EC2)
        "/ip4/54.224.107.75/tcp/4001/p2p/12D3KooWLm6braaLmNsY8X2fS8quKFmeoSxokkuRPmeh8vEt77tp".to_string(),
        "/ip4/54.224.107.75/udp/9092/quic-v1/p2p/12D3KooWLm6braaLmNsY8X2fS8quKFmeoSxokkuRPmeh8vEt77tp".to_string(),
    ];

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
        credit_nonce: Arc::new(AtomicU64::new(1)),
        scout_penalties: Arc::new(Mutex::new(ScoutPenaltyBook::default())),
        backward_passes: Arc::new(Mutex::new(VecDeque::new())),
        layer_routes: Arc::new(Mutex::new(LayerRoutingTable::default())),
        race_router: Arc::new(Mutex::new(RaceRouter::default())),
        ledger: Arc::new(Mutex::new(LedgerState::default())),
        browser_sessions: Arc::new(Mutex::new(HashMap::new())),
        browser_work: Arc::new(Mutex::new(VecDeque::new())),
        node_wallet: node_wallet.clone(),
        model_id: cli.model_id.clone(),
        layer_start: cli.layer_start,
        layer_end: cli.layer_end,
        race_pool_size: cli.race_pool_size,
        race_timeout_ms: cli.race_timeout_ms,
    };

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
    let layer_ttl_ms: u128 = 60_000;
    let mut next_layer_announcement_ms = 0u128;

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
                                    if let Err(e) = ledger.apply_signed_tx(tx) {
                                        tracing::warn!(%e, "failed to apply credit transaction");
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
                                            let _ = ledger.apply_signed_tx(tx.clone());
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

                        let topo_json = serde_json::json!({
                            "shard_peer_id": topo.local_peer_id,
                            "shard_webrtc_multiaddr": topo.webrtc_addr,
                            "shard_quic_multiaddr": topo.quic_addr,
                            "shard_ws_multiaddr": topo.ws_addr,
                            "listen_addrs": topo.listen_addrs,
                            "public_api": topo.is_public,
                            "public_api_addr": topo.public_api_addr,
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
        should_reject_peer_connection, unique_addrs, validate_work_request, LatencyHistogram,
        ScoutPenaltyBook, ScoutPenaltyUpdate, WorkRequest,
    };

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
}
