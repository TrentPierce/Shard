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
    extract::State as AxumState,
    http::Method,
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use libp2p::{
    autonat,
    dcutr,
    futures::StreamExt,
    gossipsub::{self, IdentTopic, MessageAuthenticity},
    identify,
    identity,
    kad::{store::MemoryStore, Behaviour as KadBehaviour},
    ping,
    relay,
    request_response::{self, OutboundRequestId, ProtocolSupport},
    swarm::{NetworkBehaviour, SwarmEvent},
    Multiaddr, PeerId, StreamProtocol,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    net::SocketAddr,
    path::Path,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, Mutex};
use tower_http::cors::{Any, CorsLayer};

// ─── CLI ────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug, Clone)]
#[command(name = "shard-daemon", version, about = "Shard P2P Daemon")]
struct Cli {
    /// Port for the embedded HTTP control-plane API
    #[arg(long, default_value = "9091")]
    control_port: u16,

    /// TCP transport listen port
    #[arg(long, default_value = "4001")]
    tcp_port: u16,

    /// UDP port for WebRTC-direct (non-Windows only)
    #[arg(long, default_value = "9090")]
    webrtc_port: u16,

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkResponse {
    pub request_id: String,
    pub peer_id: String,
    pub draft_tokens: Vec<String>,
    pub latency_ms: f32,
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
    ws_addr: Option<String>,
    public_api_addr: Option<String>,
    is_public: bool,
    relay_server_enabled: bool,
    contribute_enabled: bool,
    capacity: u32,        // tokens per second
    load: u32,            // current active requests
    latency_ms: f32,      // average response latency in ms
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
    work_tx: mpsc::Sender<WorkRequest>,
    daemon_start: u128,
    capacity: Arc<AtomicU32>,
    current_load: Arc<AtomicU32>,
    avg_latency_ms: Arc<AtomicU32>,
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
        "shard_ws_multiaddr": topo.ws_addr,
        "listen_addrs": topo.listen_addrs,
        "known_peer_count": known.len(),
        "public_api": topo.is_public,
        "public_api_addr": topo.public_api_addr,
        "relay_server": topo.relay_server_enabled,
        "contribute": topo.contribute_enabled,
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

async fn broadcast_work_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<WorkRequest>,
) -> Json<serde_json::Value> {
    if let Err(detail) = validate_work_request(&req) {
        return Json(serde_json::json!({ "ok": false, "detail": detail }));
    }

    match state.work_tx.send(req).await {
        Ok(_) => Json(serde_json::json!({ "ok": true, "detail": "queued for gossipsub publish" })),
        Err(e) => Json(serde_json::json!({ "ok": false, "detail": format!("channel error: {e}") })),
    }
}

async fn pop_result_handler(AxumState(state): AxumState<SharedState>) -> Json<serde_json::Value> {
    let mut results = state.results.lock().await;
    match results.pop_front() {
        Some(r) => Json(serde_json::json!({ "result": r })),
        None => Json(serde_json::json!({ "result": null })),
    }
}

fn create_router(state: SharedState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health_handler))
        .route("/topology", get(topology_handler))
        .route("/peers", get(peers_handler))
        .route("/broadcast-work", post(broadcast_work_handler))
        .route("/pop-result", get(pop_result_handler))
        .layer(cors)
        .with_state(state)
}

// ─── Main ───────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&cli.log_level)),
        )
        .init();

    // Ensure data directory exists
    let data = data_dir();
    tokio::fs::create_dir_all(&data).await?;

    let topo_path = data.join("topology.json");
    let known_peers_path = data.join("known_peers.json");

    let file_bootstrap = if let Some(path) = &cli.bootstrap_file {
        read_bootstrap_file(path).await
    } else {
        Vec::new()
    };
    let persisted_bootstrap = load_persisted_peers(&known_peers_path).await;
    let bootstrap_addrs = unique_addrs(
        cli.bootstrap
            .iter()
            .cloned()
            .chain(file_bootstrap)
            .chain(persisted_bootstrap)
            .collect(),
    );

    // ── channels ──
    let (work_tx, mut work_rx) = mpsc::channel::<WorkRequest>(256);

    let id_keys = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(id_keys.public());

    let state = SharedState {
        topology: Arc::new(Mutex::new(TopologyState {
            local_peer_id: local_peer_id.to_string(),
            listen_addrs: Vec::new(),
            webrtc_addr: None,
            ws_addr: None,
            public_api_addr: cli.public_host.clone(),
            is_public: cli.public_api,
            relay_server_enabled: cli.relay_server,
            contribute_enabled: cli.contribute,
            capacity: 100,  // Default: 100 tokens/sec
            load: 0,
            latency_ms: 0.0,
        })),
        peers: Arc::new(Mutex::new(HashMap::new())),
        known_peers: Arc::new(Mutex::new(bootstrap_addrs.clone())),
        results: Arc::new(Mutex::new(VecDeque::new())),
        work_tx,
        daemon_start: now_ms(),
        capacity: Arc::new(AtomicU32::new(100)),  // Default: 100 tokens/sec
        current_load: Arc::new(AtomicU32::new(0)),
        avg_latency_ms: Arc::new(AtomicU32::new(0)),
    };

    // ── build swarm ──
    // ── build transport ──
    let tcp_config = libp2p::tcp::Config::default().nodelay(true);
    let dns_tcp = libp2p::dns::tokio::Transport::system(libp2p::tcp::tokio::Transport::new(tcp_config.clone()))?;
    let ws_dns_tcp = libp2p::websocket::Config::new(libp2p::dns::tokio::Transport::system(libp2p::tcp::tokio::Transport::new(tcp_config))?);
    
    let tcp_ws = libp2p::core::transport::OrTransport::new(dns_tcp, ws_dns_tcp);
    
    let authenticated_transport = tcp_ws
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(libp2p::noise::Config::new(&id_keys).expect("Noise config failed"))
        .multiplex(libp2p::yamux::Config::default());
        
    let webrtc_cert = libp2p_webrtc::tokio::Certificate::generate(&mut rand::thread_rng())?;
    let webrtc = libp2p_webrtc::tokio::Transport::new(id_keys.clone(), webrtc_cert);
    
    use libp2p::Transport;
    let transport = authenticated_transport.or_transport(webrtc)
        .map(|either, _| match either {
            libp2p::futures::future::Either::Left((peer_id, muxer)) => (peer_id, libp2p::core::muxing::StreamMuxerBox::new(muxer)),
            libp2p::futures::future::Either::Right((peer_id, muxer)) => (peer_id, libp2p::core::muxing::StreamMuxerBox::new(muxer)),
        })
        .boxed();

    // ── build swarm ──
    let behaviour = {
        let gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(id_keys.clone()),
            gossipsub::Config::default(),
        ).map_err(|e| anyhow::anyhow!(e.to_string()))?;
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
    
    let mut swarm = libp2p::Swarm::new(transport, behaviour, local_peer_id, libp2p::swarm::Config::with_tokio_executor());

    // ── gossipsub topics ──
    let work_topic = IdentTopic::new("shard-work");
    let result_topic = IdentTopic::new("shard-work-result");
    let auction_topic = IdentTopic::new("auction.prompt");
    swarm.behaviour_mut().gossipsub.subscribe(&work_topic)?;
    swarm.behaviour_mut().gossipsub.subscribe(&result_topic)?;
    swarm.behaviour_mut().gossipsub.subscribe(&auction_topic)?;

    // ── listen addresses ──
    let tcp_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", cli.tcp_port).parse()?;
    let ws_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}/ws", cli.tcp_port + 100).parse()?;
    swarm.listen_on(tcp_addr)?;
    swarm.listen_on(ws_addr)?;
    let webrtc_addr: Multiaddr = format!("/ip4/0.0.0.0/udp/{}/webrtc-direct", cli.webrtc_port).parse()?;
    swarm.listen_on(webrtc_addr)?;

    // ── bootstrap peers ──
    for addr_str in &bootstrap_addrs {
        if let Ok(addr) = addr_str.parse::<Multiaddr>() {
            tracing::info!(%addr, "dialing bootstrap peer");
            let _ = swarm.dial(addr);
        }
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
        "  ║  TCP          : /ip4/0.0.0.0/tcp/{}        ║",
        cli.tcp_port
    );
    println!(
        "  ║  WebSocket    : /ip4/0.0.0.0/tcp/{}/ws   ║",
        cli.tcp_port + 100
    );
    println!(
        "  ║  Public API   : {}                              ║",
        if cli.public_api { "enabled" } else { "disabled" }
    );
    println!(
        "  ║  Public Host  : {}                       ║",
        cli.public_host.as_deref().unwrap_or("auto-detect")
    );
    println!(
        "  ║  Relay Server : {}                              ║",
        if cli.relay_server { "enabled" } else { "disabled" }
    );
    println!(
        "  ║  WebRTC       : /ip4/0.0.0.0/udp/{}/p2p-webrtc-direct ║",
        cli.webrtc_port
    );
    println!(
        "  ║  Contribute   : {}                              ║",
        if cli.contribute { "enabled" } else { "disabled" }
    );
    println!("  ╚══════════════════════════════════════════════╝");
    println!();

    let mut reconnect_tick = tokio::time::interval(Duration::from_secs(cli.reconnect_seconds));
    let mut pending_handshakes: HashMap<OutboundRequestId, PeerId> = HashMap::new();

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
            }

            // ── inbound work from Python driver (HTTP → gossipsub) ──
            Some(work_req) = work_rx.recv() => {
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

            // ── swarm events ──
            event = swarm.select_next_some() => {
                match event {
                    // ── gossipsub ──
                    SwarmEvent::Behaviour(ShardBehaviourEvent::Gossipsub(gossipsub::Event::Message { message, .. })) => {
                        if message.topic == result_topic.hash() {
                            if let Ok(result) = serde_json::from_slice::<WorkResponse>(&message.data) {
                                tracing::info!(
                                    request_id = %result.request_id,
                                    peer = %result.peer_id,
                                    tokens = result.draft_tokens.len(),
                                    "received WorkResponse via gossipsub"
                                );
                                let mut q = state.results.lock().await;
                                q.push_back(result);
                                while q.len() > 128 { q.pop_front(); }
                            }
                        }
                    }

                    // ── request/response: work forwarding ──
                    SwarmEvent::Behaviour(ShardBehaviourEvent::ControlWork(
                        request_response::Event::Message { message, .. },
                    )) => {
                        if let request_response::Message::Request { request, channel, .. } = message {
                            tracing::info!(id = %request.request_id, "work request via req/resp → publishing to gossipsub");
                            if let Ok(payload) = serde_json::to_vec(&request) {
                                let _ = swarm.behaviour_mut().gossipsub.publish(work_topic.clone(), payload);
                            }
                            let _ = swarm.behaviour_mut().control_work.send_response(
                                channel,
                                "published shard-work".to_string(),
                            );
                        }
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
                        tracing::debug!(?event, "kademlia event");
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
                    SwarmEvent::Behaviour(ShardBehaviourEvent::Autonat(event)) => {
                        match event {
                            autonat::Event::StatusChanged { old, new } => {
                                tracing::info!(?old, ?new, "AutoNAT status changed");
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

                        let topo_json = serde_json::json!({
                            "shard_peer_id": topo.local_peer_id,
                            "shard_webrtc_multiaddr": topo.webrtc_addr,
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
    use super::{unique_addrs, validate_work_request, WorkRequest};

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
    fn work_request_validation_enforces_bounds() {
        let ok = WorkRequest {
            request_id: "abc".into(),
            prompt_context: "hello".into(),
            min_tokens: 4,
        };
        assert!(validate_work_request(&ok).is_ok());

        let bad = WorkRequest {
            request_id: "".into(),
            prompt_context: "hello".into(),
            min_tokens: 0,
        };
        assert!(validate_work_request(&bad).is_err());
    }
}
