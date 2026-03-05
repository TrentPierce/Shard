//! Race router with adaptive per-peer EWMA timeouts.
//!
//! Instead of using a static `expires_at_ms`, the race timeout is calculated
//! per-peer from an Exponentially Weighted Moving Average (EWMA) of that peer's
//! historical response times. This avoids penalising fast peers for slow ones
//! and avoids giving up on slow-but-reliable peers too early.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

// ─── Adaptive Timeout ──────────────────────────────────────────────────────

/// Per-peer EWMA latency tracker for adaptive race timeouts.
#[derive(Debug, Clone)]
pub struct AdaptiveTimeout {
    /// Per-peer EWMA latency in ms.
    peer_ewma: HashMap<String, f64>,
    /// EWMA smoothing factor (0.0–1.0). Default 0.3 = 30% weight to new observation.
    alpha: f64,
    /// Hard ceiling in ms — no timeout will exceed this regardless of EWMA.
    max_timeout_ms: u64,
    /// Minimum timeout in ms — floor to avoid timing out too aggressively.
    min_timeout_ms: u64,
    /// Multiplier applied to EWMA to get the actual timeout.
    /// Default 2.0 = allow 2x the average latency before timing out.
    multiplier: f64,
    /// Threshold: if EWMA > 80% of max_timeout_ms, peer is marked slow.
    slow_threshold_ratio: f64,
}

impl AdaptiveTimeout {
    pub fn new(max_timeout_ms: u64) -> Self {
        Self {
            peer_ewma: HashMap::new(),
            alpha: 0.3,
            max_timeout_ms,
            min_timeout_ms: 50,
            multiplier: 2.0,
            slow_threshold_ratio: 0.8,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(150)
    }

    /// Record a response latency for a peer and update their EWMA.
    pub fn record_latency(&mut self, peer_id: &str, latency_ms: f64) {
        let ewma = self
            .peer_ewma
            .entry(peer_id.to_string())
            .or_insert(latency_ms);
        *ewma = self.alpha * latency_ms + (1.0 - self.alpha) * *ewma;
    }

    /// Compute the adaptive timeout for a specific peer.
    /// Returns `multiplier * EWMA`, clamped to [min_timeout_ms, max_timeout_ms].
    pub fn timeout_for_peer(&self, peer_id: &str) -> u64 {
        let ewma = self
            .peer_ewma
            .get(peer_id)
            .copied()
            .unwrap_or(self.max_timeout_ms as f64 / self.multiplier);

        let raw = (ewma * self.multiplier) as u64;
        raw.clamp(self.min_timeout_ms, self.max_timeout_ms)
    }

    /// Compute the timeout for a pool of peers (take the max of all individual timeouts).
    pub fn timeout_for_pool(&self, peer_ids: &[String]) -> u64 {
        if peer_ids.is_empty() {
            return self.max_timeout_ms;
        }
        peer_ids
            .iter()
            .map(|p| self.timeout_for_peer(p))
            .max()
            .unwrap_or(self.max_timeout_ms)
    }

    /// Check if a peer is "slow" (EWMA > 80% of ceiling).
    pub fn is_slow(&self, peer_id: &str) -> bool {
        if let Some(&ewma) = self.peer_ewma.get(peer_id) {
            ewma > (self.max_timeout_ms as f64 * self.slow_threshold_ratio)
        } else {
            false
        }
    }

    /// Get the current EWMA for a peer (for telemetry).
    pub fn ewma_for_peer(&self, peer_id: &str) -> Option<f64> {
        self.peer_ewma.get(peer_id).copied()
    }

    /// List all peers marked as slow.
    pub fn slow_peers(&self) -> Vec<String> {
        self.peer_ewma
            .iter()
            .filter(|(_, &ewma)| ewma > (self.max_timeout_ms as f64 * self.slow_threshold_ratio))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Remove tracking for a peer (e.g., on disconnect).
    pub fn remove_peer(&mut self, peer_id: &str) {
        self.peer_ewma.remove(peer_id);
    }
}

impl Default for AdaptiveTimeout {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ─── Race Router ────────────────────────────────────────────────────────────

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
    /// Latency from race start to first valid submission.
    pub latency_ms: f64,
}

#[derive(Debug, Clone)]
struct PendingRace {
    expected_shape: Vec<usize>,
    expected_dtype: String,
    allowed_peers: HashSet<String>,
    expires_at_ms: u128,
    started_at_ms: u128,
    winner: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RaceSubmitOutcome {
    AcceptedFirst,
    RejectedLate,
    RejectedInvalid,
    UnknownRace,
    /// Race timed out — fallback should be triggered.
    TimedOut,
}

#[derive(Debug, Default)]
pub struct RaceRouter {
    pending: HashMap<RaceKey, PendingRace>,
    completed: VecDeque<CompletedRace>,
    /// Adaptive per-peer timeout tracker.
    pub adaptive_timeout: AdaptiveTimeout,
}

impl RaceRouter {
    /// Start a race with an explicit timeout.
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
                started_at_ms: expires_at_ms.saturating_sub(150), // approximate
                winner: None,
            },
        );
    }

    /// Start a race with adaptive timeout computed from the peer pool's EWMA.
    pub fn start_race_adaptive(
        &mut self,
        key: RaceKey,
        expected_shape: Vec<usize>,
        expected_dtype: impl Into<String>,
        peer_pool: Vec<String>,
        now_ms: u128,
    ) {
        let timeout = self.adaptive_timeout.timeout_for_pool(&peer_pool);
        let expires_at_ms = now_ms + timeout as u128;
        let allowed_peers = peer_pool.into_iter().collect::<HashSet<_>>();
        self.pending.insert(
            key,
            PendingRace {
                expected_shape,
                expected_dtype: expected_dtype.into(),
                allowed_peers,
                expires_at_ms,
                started_at_ms: now_ms,
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

        // Calculate latency and update EWMA for this peer
        let latency_ms = (now_ms.saturating_sub(race.started_at_ms)) as f64;
        self.adaptive_timeout
            .record_latency(source_peer_id, latency_ms);

        race.winner = Some(source_peer_id.to_string());
        self.completed.push_back(CompletedRace {
            key: key.clone(),
            winner_peer_id: source_peer_id.to_string(),
            accepted_at_ms: now_ms,
            latency_ms,
        });
        RaceSubmitOutcome::AcceptedFirst
    }

    /// Check for races that have timed out. Returns their keys for fallback.
    pub fn collect_timed_out(&mut self, now_ms: u128) -> Vec<RaceKey> {
        let timed_out: Vec<RaceKey> = self
            .pending
            .iter()
            .filter(|(_, race)| now_ms > race.expires_at_ms && race.winner.is_none())
            .map(|(key, _)| key.clone())
            .collect();
        timed_out
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

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{AdaptiveTimeout, RaceKey, RaceRouter, RaceSubmitOutcome};

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
        router.start_race(key.clone(), vec![2, 8], "fp32", vec!["p1".into()], 10_000);
        assert_eq!(
            router.submit_candidate(100, &key, "p2", &[2, 8], "fp32"),
            RaceSubmitOutcome::RejectedInvalid
        );
        assert_eq!(
            router.submit_candidate(100, &key, "p1", &[1, 8], "fp32"),
            RaceSubmitOutcome::RejectedInvalid
        );
    }

    #[test]
    fn adaptive_timeout_ewma_convergence() {
        let mut at = AdaptiveTimeout::with_defaults(); // max 150ms

        // Simulate peer "fast" with consistent 30ms latency
        for _ in 0..10 {
            at.record_latency("fast", 30.0);
        }
        // EWMA should converge toward 30ms
        let ewma = at
            .ewma_for_peer("fast")
            .expect("EWMA should exist after recording latency");
        assert!(
            (ewma - 30.0).abs() < 5.0,
            "fast peer EWMA should converge near 30ms, got {ewma}"
        );

        // Timeout should be 2x EWMA ≈ 60ms, clamped to [50, 150]
        let timeout = at.timeout_for_peer("fast");
        assert!(
            (50..=150).contains(&timeout),
            "timeout should be in [50, 150], got {timeout}"
        );
    }

    #[test]
    fn slow_peer_detection() {
        let mut at = AdaptiveTimeout::with_defaults(); // max 150ms, slow threshold 80% = 120ms

        // Simulate peer with 130ms average — should be marked slow
        for _ in 0..20 {
            at.record_latency("slow_peer", 130.0);
        }
        assert!(at.is_slow("slow_peer"));

        // Fast peer should not be slow
        for _ in 0..20 {
            at.record_latency("fast_peer", 20.0);
        }
        assert!(!at.is_slow("fast_peer"));
    }

    #[test]
    fn adaptive_race_start() {
        let mut router = RaceRouter::default();
        // Give peer_a a known EWMA
        router.adaptive_timeout.record_latency("peer_a", 40.0);
        router.adaptive_timeout.record_latency("peer_a", 40.0);
        router.adaptive_timeout.record_latency("peer_a", 40.0);

        let key = RaceKey {
            request_id: "r3".into(),
            step_id: "s3".into(),
        };
        router.start_race_adaptive(
            key.clone(),
            vec![1, 4],
            "fp16",
            vec!["peer_a".into()],
            1000, // now_ms
        );

        // Peer responds at 1050ms (50ms later)
        let result = router.submit_candidate(1050, &key, "peer_a", &[1, 4], "fp16");
        assert_eq!(result, RaceSubmitOutcome::AcceptedFirst);

        // Check the completed race has latency recorded
        let completed = router
            .pop_completed(Some(&key))
            .expect("completed race should be available");
        assert!((completed.latency_ms - 50.0).abs() < 1.0);
    }

    #[test]
    fn collect_timed_out_races() {
        let mut router = RaceRouter::default();
        let key1 = RaceKey {
            request_id: "r1".into(),
            step_id: "s1".into(),
        };
        let key2 = RaceKey {
            request_id: "r2".into(),
            step_id: "s2".into(),
        };

        router.start_race(key1.clone(), vec![1], "fp16", vec!["p1".into()], 100);
        router.start_race(key2.clone(), vec![1], "fp16", vec!["p1".into()], 200);

        // At t=150, key1 should be timed out but key2 still pending
        let timed_out = router.collect_timed_out(150);
        assert_eq!(timed_out.len(), 1);
        assert_eq!(timed_out[0], key1);
    }

    #[test]
    fn unknown_peer_gets_default_timeout() {
        let at = AdaptiveTimeout::with_defaults();
        // Unknown peer should get max_timeout_ms / multiplier * multiplier = max_timeout_ms
        let timeout = at.timeout_for_peer("unknown");
        assert_eq!(timeout, 150); // clamped to max
    }

    #[test]
    fn pool_timeout_is_max_of_individuals() {
        let mut at = AdaptiveTimeout::with_defaults();
        at.record_latency("fast", 20.0);
        at.record_latency("slow", 60.0);

        let pool_timeout = at.timeout_for_pool(&["fast".into(), "slow".into()]);
        let fast_timeout = at.timeout_for_peer("fast");
        let slow_timeout = at.timeout_for_peer("slow");

        assert_eq!(pool_timeout, fast_timeout.max(slow_timeout));
    }
}
