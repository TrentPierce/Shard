use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeReputation {
    pub success_count: u64,
    pub failure_count: u64,
    pub avg_latency_ms: f64,
    pub identity_score: f64,
    pub hardware_capability_score: f64,
    pub last_updated_ms: u128,
}

impl NodeReputation {
    pub fn reliability_score(&self) -> f64 {
        let total = self.success_count.saturating_add(self.failure_count);
        if total == 0 {
            return 1.0;
        }
        self.success_count as f64 / total as f64
    }

    pub fn update_success(&mut self, latency_ms: f64, now_ms: u128) {
        self.success_count = self.success_count.saturating_add(1);
        if self.avg_latency_ms <= 0.0 {
            self.avg_latency_ms = latency_ms.max(0.0);
        } else {
            self.avg_latency_ms = (self.avg_latency_ms * 0.8) + (latency_ms.max(0.0) * 0.2);
        }
        self.identity_score = (self.identity_score + 0.01).clamp(0.0, 1.0);
        self.last_updated_ms = now_ms;
    }

    pub fn update_failure(&mut self, now_ms: u128) {
        self.failure_count = self.failure_count.saturating_add(1);
        self.identity_score = (self.identity_score - 0.02).clamp(0.0, 1.0);
        self.last_updated_ms = now_ms;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeSchedulerInput {
    pub node_id: String,
    pub load: f64,
    pub latency_ms: f64,
    pub reliability_score: f64,
    pub hardware_capability_score: f64,
    pub identity_reputation_score: f64,
}

impl NodeSchedulerInput {
    pub fn weight(&self) -> f64 {
        // Higher weight is better.
        let load_factor = (1.0 - self.load).clamp(0.0, 1.0);
        let latency_factor = (1.0 / (1.0 + self.latency_ms.max(0.0) / 200.0)).clamp(0.0, 1.0);
        let reliability = self.reliability_score.clamp(0.0, 1.0);
        let hardware = self.hardware_capability_score.clamp(0.1, 1.0);
        let identity = self.identity_reputation_score.clamp(0.0, 1.0);
        (0.30 * load_factor)
            + (0.20 * latency_factor)
            + (0.25 * reliability)
            + (0.15 * hardware)
            + (0.10 * identity)
    }
}

pub fn weighted_select(mut candidates: Vec<NodeSchedulerInput>, limit: usize) -> Vec<String> {
    candidates.sort_by(|a, b| {
        b.weight()
            .total_cmp(&a.weight())
            .then_with(|| a.node_id.cmp(&b.node_id))
    });
    candidates
        .into_iter()
        .take(limit)
        .map(|item| item.node_id)
        .collect()
}

pub fn load_reputation(path: &Path) -> HashMap<String, NodeReputation> {
    let Ok(bytes) = std::fs::read(path) else {
        return HashMap::new();
    };
    serde_json::from_slice::<HashMap<String, NodeReputation>>(&bytes).unwrap_or_default()
}

pub fn save_reputation(path: &Path, reputation: &HashMap<String, NodeReputation>) {
    if let Ok(payload) = serde_json::to_vec_pretty(reputation) {
        let _ = std::fs::write(path, payload);
    }
}

#[cfg(test)]
mod tests {
    use super::{weighted_select, NodeSchedulerInput};

    #[test]
    fn weighted_scheduler_prefers_reliable_low_latency_nodes() {
        let out = weighted_select(
            vec![
                NodeSchedulerInput {
                    node_id: "a".into(),
                    load: 0.6,
                    latency_ms: 300.0,
                    reliability_score: 0.6,
                    hardware_capability_score: 0.6,
                    identity_reputation_score: 0.8,
                },
                NodeSchedulerInput {
                    node_id: "b".into(),
                    load: 0.1,
                    latency_ms: 35.0,
                    reliability_score: 0.95,
                    hardware_capability_score: 0.8,
                    identity_reputation_score: 0.9,
                },
                NodeSchedulerInput {
                    node_id: "c".into(),
                    load: 0.3,
                    latency_ms: 120.0,
                    reliability_score: 0.8,
                    hardware_capability_score: 0.7,
                    identity_reputation_score: 0.85,
                },
            ],
            2,
        );

        assert_eq!(out, vec!["b".to_string(), "c".to_string()]);
    }
}
