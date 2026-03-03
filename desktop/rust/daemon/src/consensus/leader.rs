use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConsensusRole {
    Follower,
    Candidate,
    Leader,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusSnapshot {
    pub role: ConsensusRole,
    pub term: u64,
    pub leader_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ElectionMessage {
    RequestVote { term: u64, candidate_id: String },
    Vote {
        term: u64,
        voter_id: String,
        candidate_id: String,
        granted: bool,
    },
    Heartbeat { term: u64, leader_id: String },
    NodeLeaving { node_id: String, peer_id: String },
}

#[derive(Debug, Deserialize)]
struct ConsensusYaml {
    consensus: Option<ConsensusYamlInner>,
}

#[derive(Debug, Deserialize)]
struct ConsensusYamlInner {
    heartbeat_interval_seconds: Option<u64>,
    heartbeat_timeout_seconds: Option<u64>,
    election_timeout_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct LeaderElectionConfig {
    pub heartbeat_interval_seconds: u64,
    pub heartbeat_timeout_seconds: u64,
    pub election_timeout_ms: u64,
}

impl Default for LeaderElectionConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_seconds: 2,
            heartbeat_timeout_seconds: 6,
            election_timeout_ms: 150,
        }
    }
}

impl LeaderElectionConfig {
    pub fn from_config(path: &Path) -> Self {
        let Ok(raw) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        let Ok(parsed) = serde_yaml::from_str::<ConsensusYaml>(&raw) else {
            return Self::default();
        };
        let inner = parsed.consensus.unwrap_or(ConsensusYamlInner {
            heartbeat_interval_seconds: None,
            heartbeat_timeout_seconds: None,
            election_timeout_ms: None,
        });
        Self {
            heartbeat_interval_seconds: inner.heartbeat_interval_seconds.unwrap_or(2).max(1),
            heartbeat_timeout_seconds: inner.heartbeat_timeout_seconds.unwrap_or(6).max(2),
            election_timeout_ms: inner.election_timeout_ms.unwrap_or(150).max(50),
        }
    }

    fn randomized_election_timeout(&self) -> Duration {
        let mut rng = rand::thread_rng();
        let low = self.election_timeout_ms;
        let high = self.election_timeout_ms.saturating_mul(2).max(low + 1);
        Duration::from_millis(rng.gen_range(low..=high))
    }
}

#[derive(Debug)]
pub enum LeaderInput {
    NetworkMessage {
        from_peer: Option<String>,
        message: ElectionMessage,
    },
    PeerCount(usize),
}

#[derive(Clone)]
pub struct LeaderElectionHandle {
    snapshot: Arc<RwLock<ConsensusSnapshot>>,
    input_tx: mpsc::Sender<LeaderInput>,
}

impl LeaderElectionHandle {
    pub async fn snapshot(&self) -> ConsensusSnapshot {
        self.snapshot.read().await.clone()
    }

    pub fn input_sender(&self) -> mpsc::Sender<LeaderInput> {
        self.input_tx.clone()
    }
}

pub fn spawn_leader_election(
    local_peer_id: String,
    config: LeaderElectionConfig,
) -> (LeaderElectionHandle, mpsc::Receiver<ElectionMessage>) {
    let snapshot = Arc::new(RwLock::new(ConsensusSnapshot {
        role: ConsensusRole::Follower,
        term: 0,
        leader_id: None,
    }));
    let (input_tx, mut input_rx) = mpsc::channel::<LeaderInput>(256);
    let (output_tx, output_rx) = mpsc::channel::<ElectionMessage>(256);

    let snapshot_ref = Arc::clone(&snapshot);
    tokio::spawn(async move {
        let mut term: u64 = 0;
        let mut role = ConsensusRole::Follower;
        let mut voted_for: Option<String> = None;
        let mut leader_id: Option<String> = None;
        let mut votes_received: HashSet<String> = HashSet::new();
        let mut known_peer_count = 0usize;
        let mut last_heartbeat = Instant::now();
        let mut last_heartbeat_sent = Instant::now();
        let mut election_deadline = Instant::now() + config.randomized_election_timeout();

        let mut tick = tokio::time::interval(Duration::from_millis(100));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                maybe_input = input_rx.recv() => {
                    let Some(input) = maybe_input else {
                        break;
                    };
                    match input {
                        LeaderInput::PeerCount(count) => {
                            known_peer_count = count;
                        }
                        LeaderInput::NetworkMessage { from_peer, message } => {
                            match message {
                                ElectionMessage::Heartbeat { term: remote_term, leader_id: remote_leader_id } => {
                                    if remote_term >= term {
                                        term = remote_term;
                                        role = ConsensusRole::Follower;
                                        leader_id = Some(remote_leader_id);
                                        voted_for = None;
                                        votes_received.clear();
                                        last_heartbeat = Instant::now();
                                        election_deadline = Instant::now() + config.randomized_election_timeout();
                                    }
                                }
                                ElectionMessage::RequestVote { term: remote_term, candidate_id } => {
                                    if remote_term > term {
                                        term = remote_term;
                                        role = ConsensusRole::Follower;
                                        voted_for = None;
                                        leader_id = None;
                                        votes_received.clear();
                                    }
                                    let mut granted = false;
                                    if remote_term == term {
                                        let not_voted = voted_for.is_none();
                                        let same_candidate = voted_for.as_deref() == Some(candidate_id.as_str());
                                        if not_voted || same_candidate {
                                            granted = true;
                                            voted_for = Some(candidate_id.clone());
                                            election_deadline = Instant::now() + config.randomized_election_timeout();
                                        }
                                    }
                                    let voter_id = local_peer_id.clone();
                                    let vote = ElectionMessage::Vote {
                                        term,
                                        voter_id,
                                        candidate_id,
                                        granted,
                                    };
                                    let _ = output_tx.send(vote).await;
                                }
                                ElectionMessage::Vote { term: remote_term, voter_id, candidate_id, granted } => {
                                    if remote_term > term {
                                        term = remote_term;
                                        role = ConsensusRole::Follower;
                                        voted_for = None;
                                        leader_id = None;
                                        votes_received.clear();
                                    } else if role == ConsensusRole::Candidate && remote_term == term && granted && candidate_id == local_peer_id {
                                        let voter = from_peer.unwrap_or(voter_id);
                                        votes_received.insert(voter);
                                        let quorum = ((known_peer_count + 1) / 2) + 1;
                                        if votes_received.len() >= quorum {
                                            role = ConsensusRole::Leader;
                                            leader_id = Some(local_peer_id.clone());
                                            votes_received.clear();
                                            last_heartbeat_sent = Instant::now() - Duration::from_secs(config.heartbeat_interval_seconds);
                                        }
                                    }
                                }
                                ElectionMessage::NodeLeaving { peer_id, .. } => {
                                    if leader_id.as_deref() == Some(peer_id.as_str()) {
                                        leader_id = None;
                                        role = ConsensusRole::Follower;
                                        election_deadline = Instant::now() + config.randomized_election_timeout();
                                    }
                                }
                            }
                        }
                    }
                }
                _ = tick.tick() => {
                    match role {
                        ConsensusRole::Leader => {
                            if last_heartbeat_sent.elapsed() >= Duration::from_secs(config.heartbeat_interval_seconds) {
                                let heartbeat = ElectionMessage::Heartbeat {
                                    term,
                                    leader_id: local_peer_id.clone(),
                                };
                                let _ = output_tx.send(heartbeat).await;
                                last_heartbeat_sent = Instant::now();
                            }
                        }
                        ConsensusRole::Follower | ConsensusRole::Candidate => {
                            let heartbeat_deadline = Duration::from_secs(config.heartbeat_timeout_seconds.saturating_mul(3));
                            let no_heartbeat_too_long = last_heartbeat.elapsed() >= heartbeat_deadline;
                            if no_heartbeat_too_long || Instant::now() >= election_deadline {
                                term = term.saturating_add(1);
                                role = ConsensusRole::Candidate;
                                voted_for = Some(local_peer_id.clone());
                                leader_id = None;
                                votes_received.clear();
                                votes_received.insert(local_peer_id.clone());

                                let request = ElectionMessage::RequestVote {
                                    term,
                                    candidate_id: local_peer_id.clone(),
                                };
                                let _ = output_tx.send(request).await;
                                election_deadline = Instant::now() + config.randomized_election_timeout();

                                let quorum = ((known_peer_count + 1) / 2) + 1;
                                if votes_received.len() >= quorum {
                                    role = ConsensusRole::Leader;
                                    leader_id = Some(local_peer_id.clone());
                                    votes_received.clear();
                                    last_heartbeat_sent = Instant::now() - Duration::from_secs(config.heartbeat_interval_seconds);
                                }
                            }
                        }
                    }
                }
            }

            let mut snap = snapshot_ref.write().await;
            snap.role = role;
            snap.term = term;
            snap.leader_id = leader_id.clone();
        }
    });

    (
        LeaderElectionHandle { snapshot, input_tx },
        output_rx,
    )
}

#[cfg(test)]
mod tests {
    use super::{spawn_leader_election, ConsensusRole, ElectionMessage, LeaderInput};
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn elects_self_with_no_peers() {
        let (handle, _out) = spawn_leader_election("peer-a".to_string(), Default::default());
        let tx = handle.input_sender();
        let _ = tx.send(LeaderInput::PeerCount(0)).await;
        sleep(Duration::from_millis(400)).await;
        let snap = handle.snapshot().await;
        assert!(matches!(snap.role, ConsensusRole::Leader | ConsensusRole::Candidate));
    }

    #[tokio::test]
    async fn heartbeat_updates_leader() {
        let (handle, _out) = spawn_leader_election("peer-b".to_string(), Default::default());
        let tx = handle.input_sender();
        let _ = tx
            .send(LeaderInput::NetworkMessage {
                from_peer: Some("peer-z".to_string()),
                message: ElectionMessage::Heartbeat {
                    term: 3,
                    leader_id: "peer-z".to_string(),
                },
            })
            .await;
        sleep(Duration::from_millis(50)).await;
        let snap = handle.snapshot().await;
        assert_eq!(snap.term, 3);
        assert_eq!(snap.leader_id.as_deref(), Some("peer-z"));
        assert_eq!(snap.role, ConsensusRole::Follower);
    }
}
