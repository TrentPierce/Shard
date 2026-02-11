//! Shard Oracle Daemon — P2P networking sidecar for the Shard inference network.
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
    futures::StreamExt,
    gossipsub::{self, IdentTopic, MessageAuthenticity},
    identity,
    kad::{store::MemoryStore, Behaviour as KadBehaviour},
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, SwarmEvent},
    Multiaddr, PeerId, StreamProtocol, SwarmBuilder,
};
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, Mutex};
use tower_http::cors::{Any, CorsLayer};

// ─── CLI ────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug, Clone)]
#[command(name = "shard-daemon", version, about = "Shard Oracle P2P Daemon")]
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

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
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

// ─── Shared State ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize)]
struct TopologyState {
    local_peer_id: String,
    listen_addrs: Vec<String>,
    webrtc_addr: Option<String>,
    ws_addr: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct PeerInfo {
    peer_id: String,
    connected_at: u128,
    addrs: Vec<String>,
}

#[derive(Clone)]
struct SharedState {
    topology: Arc<Mutex<TopologyState>>,
    peers: Arc<Mutex<HashMap<String, PeerInfo>>>,
    results: Arc<Mutex<VecDeque<WorkResponse>>>,
    work_tx: mpsc::Sender<WorkRequest>,
    daemon_start: u128,
}

// ─── libp2p Behaviour ───────────────────────────────────────────────────────

#[derive(NetworkBehaviour)]
struct OracleBehaviour {
    gossipsub: gossipsub::Behaviour,
    kad: KadBehaviour<MemoryStore>,
    handshake: request_response::cbor::Behaviour<Heartbeat, Heartbeat>,
    verify: request_response::cbor::Behaviour<DraftSubmission, String>,
    control_work: request_response::cbor::Behaviour<WorkRequest, String>,
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

// ─── HTTP Control-Plane Handlers ────────────────────────────────────────────

async fn health_handler(AxumState(state): AxumState<SharedState>) -> Json<serde_json::Value> {
    let topo = state.topology.lock().await;
    let peers = state.peers.lock().await;
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "peer_id": topo.local_peer_id,
        "connected_peers": peers.len(),
        "uptime_ms": now_ms() - state.daemon_start,
        "listen_addrs": topo.listen_addrs,
    }))
}

async fn topology_handler(AxumState(state): AxumState<SharedState>) -> Json<serde_json::Value> {
    let topo = state.topology.lock().await;
    Json(serde_json::json!({
        "status": "ok",
        "source": "rust-sidecar",
        "oracle_peer_id": topo.local_peer_id,
        "oracle_webrtc_multiaddr": topo.webrtc_addr,
        "oracle_ws_multiaddr": topo.ws_addr,
        "listen_addrs": topo.listen_addrs,
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
        })),
        peers: Arc::new(Mutex::new(HashMap::new())),
        results: Arc::new(Mutex::new(VecDeque::new())),
        work_tx,
        daemon_start: now_ms(),
    };

    // ── build swarm ──
    let mut swarm = SwarmBuilder::with_existing_identity(id_keys)
        .with_tokio()
        .with_tcp(
            Default::default(),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )?
        .with_websocket(libp2p::noise::Config::new, libp2p::yamux::Config::default)
        .await?
        .with_behaviour(|key| {
            let local_peer_id = PeerId::from(key.public());
            let gossipsub = gossipsub::Behaviour::new(
                MessageAuthenticity::Signed(key.clone()),
                gossipsub::Config::default(),
            )?;
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
                    StreamProtocol::new("/shard/oracle/verify/1.0.0"),
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
            Ok(OracleBehaviour {
                gossipsub,
                kad,
                handshake,
                verify,
                control_work,
            })
        })?
        .build();

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

    // ── bootstrap peers ──
    for addr_str in &cli.bootstrap {
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

    // ── persist topology hint to disk (for legacy file-based readers) ──
    let topo_path = data.join("topology.json");

    println!();
    println!("  ╔══════════════════════════════════════════════╗");
    println!("  ║       Shard Oracle Daemon  v{}           ║", env!("CARGO_PKG_VERSION"));
    println!("  ╠══════════════════════════════════════════════╣");
    println!("  ║  Peer ID      : {}…  ║", &local_peer_id.to_string()[..20]);
    println!("  ║  Control API  : http://0.0.0.0:{}          ║", control_port);
    println!("  ║  TCP          : /ip4/0.0.0.0/tcp/{}        ║", cli.tcp_port);
    println!("  ║  WebSocket    : /ip4/0.0.0.0/tcp/{}/ws   ║", cli.tcp_port + 100);
    println!("  ╚══════════════════════════════════════════════╝");
    println!();

    // ── main event loop ──
    loop {
        tokio::select! {
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
                    SwarmEvent::Behaviour(OracleBehaviourEvent::Gossipsub(gossipsub::Event::Message { message, .. })) => {
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
                                // Cap queue at 128 entries
                                while q.len() > 128 { q.pop_front(); }
                            }
                        }
                    }

                    // ── request/response: work forwarding ──
                    SwarmEvent::Behaviour(OracleBehaviourEvent::ControlWork(
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
                    SwarmEvent::Behaviour(OracleBehaviourEvent::Handshake(
                        request_response::Event::Message { peer, message, .. },
                    )) => {
                        match message {
                            request_response::Message::Request { request, channel, .. } => {
                                if request.kind == "PING" {
                                    let latency = now_ms().saturating_sub(request.sent_at_ms);
                                    tracing::info!(%peer, %latency, "PING → PONG");
                                    let pong = Heartbeat { kind: "PONG".into(), sent_at_ms: now_ms() };
                                    let _ = swarm.behaviour_mut().handshake.send_response(channel, pong);
                                }
                            }
                            request_response::Message::Response { response, .. } => {
                                tracing::info!(%peer, kind = %response.kind, "handshake response");
                            }
                        }
                    }

                    // ── verify protocol ──
                    SwarmEvent::Behaviour(OracleBehaviourEvent::Verify(event)) => {
                        tracing::debug!(?event, "verify protocol event");
                    }

                    // ── kademlia ──
                    SwarmEvent::Behaviour(OracleBehaviourEvent::Kad(event)) => {
                        tracing::debug!(?event, "kademlia event");
                    }

                    // ── new listen addresses → update topology ──
                    SwarmEvent::NewListenAddr { address, .. } => {
                        tracing::info!(%address, "listening on");
                        let addr_str = address.to_string();
                        let mut topo = state.topology.lock().await;
                        topo.listen_addrs.push(addr_str.clone());

                        if addr_str.contains("/ws") {
                            topo.ws_addr = Some(format!("{}/p2p/{}", addr_str, local_peer_id));
                        }
                        if addr_str.contains("/webrtc-direct/") {
                            topo.webrtc_addr = Some(format!("{}/p2p/{}", addr_str, local_peer_id));
                        }

                        // Persist topology hint
                        let topo_json = serde_json::json!({
                            "oracle_peer_id": topo.local_peer_id,
                            "oracle_webrtc_multiaddr": topo.webrtc_addr,
                            "oracle_ws_multiaddr": topo.ws_addr,
                            "listen_addrs": topo.listen_addrs,
                        });
                        let _ = tokio::fs::write(&topo_path, topo_json.to_string()).await;
                    }

                    // ── peer connections ──
                    SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                        tracing::info!(%peer_id, ?endpoint, "peer connected");
                        let mut peers = state.peers.lock().await;
                        peers.insert(
                            peer_id.to_string(),
                            PeerInfo {
                                peer_id: peer_id.to_string(),
                                connected_at: now_ms(),
                                addrs: vec![endpoint.get_remote_address().to_string()],
                            },
                        );
                    }
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        tracing::info!(%peer_id, "peer disconnected");
                        let mut peers = state.peers.lock().await;
                        peers.remove(&peer_id.to_string());
                    }

                    _ => {}
                }
            }
        }
    }
}
