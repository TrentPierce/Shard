use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct RaceKey {
    pub request_id: String,
    pub step_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedRace {
    pub key: RaceKey,
    pub winner_peer_id: String,
    pub accepted_at_ms: u128,
}

#[derive(Debug, Clone)]
struct PendingRace {
    expected_shape: Vec<usize>,
    expected_dtype: String,
    allowed_peers: HashSet<String>,
    expires_at_ms: u128,
    winner: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RaceSubmitOutcome {
    AcceptedFirst,
    RejectedLate,
    RejectedInvalid,
    UnknownRace,
}

#[derive(Debug, Default)]
pub struct RaceRouter {
    pending: HashMap<RaceKey, PendingRace>,
    completed: VecDeque<CompletedRace>,
}

impl RaceRouter {
    pub fn start_race(
        &mut self,
        key: RaceKey,
        expected_shape: Vec<usize>,
        expected_dtype: impl Into<String>,
        peer_pool: Vec<String>,
        expires_at_ms: u128,
    ) {
        let allowed_peers = peer_pool.into_iter().collect::<HashSet<_>>();
        self.pending.insert(
            key,
            PendingRace {
                expected_shape,
                expected_dtype: expected_dtype.into(),
                allowed_peers,
                expires_at_ms,
                winner: None,
            },
        );
    }

    pub fn submit_candidate(
        &mut self,
        now_ms: u128,
        key: &RaceKey,
        source_peer_id: &str,
        shape: &[usize],
        dtype: &str,
    ) -> RaceSubmitOutcome {
        let Some(race) = self.pending.get_mut(key) else {
            return RaceSubmitOutcome::UnknownRace;
        };
        if now_ms > race.expires_at_ms {
            return RaceSubmitOutcome::RejectedLate;
        }
        if race.winner.is_some() {
            return RaceSubmitOutcome::RejectedLate;
        }
        if !race.allowed_peers.contains(source_peer_id)
            || race.expected_shape.as_slice() != shape
            || race.expected_dtype != dtype
        {
            return RaceSubmitOutcome::RejectedInvalid;
        }

        race.winner = Some(source_peer_id.to_string());
        self.completed.push_back(CompletedRace {
            key: key.clone(),
            winner_peer_id: source_peer_id.to_string(),
            accepted_at_ms: now_ms,
        });
        RaceSubmitOutcome::AcceptedFirst
    }

    pub fn pop_completed(&mut self, key: Option<&RaceKey>) -> Option<CompletedRace> {
        if let Some(target) = key {
            let idx = self.completed.iter().position(|r| &r.key == target)?;
            return self.completed.remove(idx);
        }
        self.completed.pop_front()
    }

    pub fn prune_expired(&mut self, now_ms: u128) {
        self.pending.retain(|_, p| p.expires_at_ms > now_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::{RaceKey, RaceRouter, RaceSubmitOutcome};

    #[test]
    fn accepts_first_valid_and_drops_late() {
        let mut router = RaceRouter::default();
        let key = RaceKey {
            request_id: "r1".into(),
            step_id: "s1".into(),
        };
        router.start_race(
            key.clone(),
            vec![1, 4],
            "fp16",
            vec!["p1".into(), "p2".into()],
            10_000,
        );

        let first = router.submit_candidate(100, &key, "p1", &[1, 4], "fp16");
        let second = router.submit_candidate(120, &key, "p2", &[1, 4], "fp16");
        assert_eq!(first, RaceSubmitOutcome::AcceptedFirst);
        assert_eq!(second, RaceSubmitOutcome::RejectedLate);
    }

    #[test]
    fn rejects_wrong_shape_or_peer() {
        let mut router = RaceRouter::default();
        let key = RaceKey {
            request_id: "r2".into(),
            step_id: "s2".into(),
        };
        router.start_race(
            key.clone(),
            vec![2, 8],
            "fp32",
            vec!["p1".into()],
            10_000,
        );
        assert_eq!(
            router.submit_candidate(100, &key, "p2", &[2, 8], "fp32"),
            RaceSubmitOutcome::RejectedInvalid
        );
        assert_eq!(
            router.submit_candidate(100, &key, "p1", &[1, 8], "fp32"),
            RaceSubmitOutcome::RejectedInvalid
        );
    }
}
