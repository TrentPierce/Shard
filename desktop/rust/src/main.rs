use anyhow::Result;
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
use std::{fs, time::{SystemTime, UNIX_EPOCH}};

const TOPOLOGY_HINT_PATH: &str = "/tmp/shard-topology.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DraftSubmission {
    task_id: String,
    scout_peer_id: String,
    seq_start: u32,
    draft_tokens: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Heartbeat {
    kind: String,
    sent_at_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkRequest {
    request_id: String,
    prompt_context: String,
    min_tokens: i32,
    sequence_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkResponse {
    request_id: String,
    peer_id: String,
    draft_tokens: Vec<String>,
    latency_ms: f32,
    sequence_id: i64,
}

#[derive(NetworkBehaviour)]
struct OracleBehaviour {
    gossipsub: gossipsub::Behaviour,
    kad: KadBehaviour<MemoryStore>,
    handshake: request_response::cbor::Behaviour<Heartbeat, Heartbeat>,
    verify: request_response::cbor::Behaviour<DraftSubmission, String>,
    control_work: request_response::cbor::Behaviour<WorkRequest, String>,
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

fn forward_result_to_python_callback(result: &WorkResponse) {
    let payload = serde_json::to_string(result).unwrap_or_else(|_| "{}".to_string());
    let _ = fs::write("/tmp/shard-work-result.json", payload);
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let id_keys = identity::Keypair::generate_ed25519();
    let cert = libp2p::webrtc::tokio::Certificate::generate(&mut thread_rng())?;

    let mut swarm = SwarmBuilder::with_existing_identity(id_keys)
        .with_tokio()
        .with_tcp(
            Default::default(),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )?
        .with_websocket(libp2p::noise::Config::new, libp2p::yamux::Config::default)
        .await?
        .with_other_transport(|key| libp2p::webrtc::tokio::Transport::new(key.clone(), cert))?
        .with_behaviour(|key| {
            let local_peer_id = PeerId::from(key.public());
            let gossipsub = gossipsub::Behaviour::new(
                MessageAuthenticity::Signed(key.clone()),
                gossipsub::Config::default(),
            )?;
            let kad = KadBehaviour::new(local_peer_id, MemoryStore::new(local_peer_id));
            let handshake = request_response::cbor::Behaviour::new(
                [(StreamProtocol::new("/shard/1.0.0/handshake"), ProtocolSupport::Full)],
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

    let auction_topic = IdentTopic::new("auction.prompt");
    let work_topic = IdentTopic::new("shard-work");
    let result_topic = IdentTopic::new("shard-work-result");
    swarm.behaviour_mut().gossipsub.subscribe(&auction_topic)?;
    swarm.behaviour_mut().gossipsub.subscribe(&work_topic)?;
    swarm.behaviour_mut().gossipsub.subscribe(&result_topic)?;

    swarm.listen_on("/ip4/0.0.0.0/tcp/4001/ws".parse::<Multiaddr>()?)?;
    swarm.listen_on("/ip4/0.0.0.0/udp/9090/webrtc-direct".parse::<Multiaddr>()?)?;

    tracing::info!("Oracle started. Waiting for PING and shard-work traffic");

    loop {
        match swarm.select_next_some().await {
            SwarmEvent::Behaviour(OracleBehaviourEvent::ControlWork(event)) => {
                if let request_response::Event::Message { message, .. } = event {
                    if let request_response::Message::Request { request, channel, .. } = message {
                        tracing::debug!(request_id = %request.request_id, sequence_id = request.sequence_id, "publishing shard-work request");
                        if let Ok(payload) = serde_json::to_vec(&request) {
                            let _ = swarm.behaviour_mut().gossipsub.publish(work_topic.clone(), payload);
                        }
                        let _ = swarm
                            .behaviour_mut()
                            .control_work
                            .send_response(channel, "published shard-work".to_string());
                    }
                }
            }
            SwarmEvent::Behaviour(OracleBehaviourEvent::Gossipsub(event)) => {
                if let gossipsub::Event::Message { message, .. } = event {
                    if message.topic == result_topic.hash() {
                        if let Ok(result) = serde_json::from_slice::<WorkResponse>(&message.data) {
                            forward_result_to_python_callback(&result);
                            tracing::info!(request_id = %result.request_id, sequence_id = result.sequence_id, "forwarded WorkResponse to python callback shim");
                        }
                    }
                }
            }
            SwarmEvent::Behaviour(OracleBehaviourEvent::Handshake(event)) => {
                match event {
                    request_response::Event::Message { peer, message, .. } => match message {
                        request_response::Message::Request { request, channel, .. } => {
                            if request.kind == "PING" {
                                let latency = now_ms().saturating_sub(request.sent_at_ms);
                                tracing::info!(%peer, %latency, "received PING; replying PONG");
                                let response = Heartbeat { kind: "PONG".to_string(), sent_at_ms: now_ms() };
                                swarm.behaviour_mut().handshake.send_response(channel, response).expect("response channel open");
                            }
                        }
                        request_response::Message::Response { response, .. } => {
                            tracing::info!(%peer, kind = %response.kind, "handshake response");
                        }
                    },
                    other => tracing::debug!(?other, "handshake event"),
                }
            }
            SwarmEvent::Behaviour(OracleBehaviourEvent::Verify(event)) => {
                tracing::debug!(?event, "verify protocol event");
            }
            SwarmEvent::Behaviour(OracleBehaviourEvent::Kad(event)) => {
                tracing::debug!(?event, "kademlia event");
            }
            SwarmEvent::NewListenAddr { address, .. } => {
                tracing::info!(%address, "oracle listening address");
                if address.to_string().contains("/webrtc-direct/") {
                    let topo = serde_json::json!({
                        "oracle_peer_hint": "local",
                        "oracle_webrtc_multiaddr": address.to_string(),
                    });
                    let _ = fs::write(TOPOLOGY_HINT_PATH, topo.to_string());
                }
            }
            _ => {}
        }
    }
}
