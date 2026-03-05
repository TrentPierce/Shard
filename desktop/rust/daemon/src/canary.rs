use super::now_ms;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CanaryRolloutConfig {
    pub(crate) enabled: bool,
    pub(crate) canary_model_id: String,
    pub(crate) traffic_percent: u8,
    pub(crate) max_avg_latency_ms: u64,
    pub(crate) min_acceptance_rate: f64,
    pub(crate) max_reject_rate: f64,
    pub(crate) min_samples: u64,
}

impl CanaryRolloutConfig {
    pub(crate) fn from_env(_default_model_id: &str) -> Self {
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
    pub(crate) rollback_active: bool,
    pub(crate) rollback_reason: Option<String>,
    pub(crate) last_evaluated_ms: u128,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub(crate) struct CanaryRolloutStats {
    pub(crate) canary_requests_total: u64,
    pub(crate) canary_latency_sum_ms: u64,
    pub(crate) canary_acceptance_samples: u64,
    pub(crate) canary_acceptance_sum: f64,
    pub(crate) canary_reject_sum: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct CanaryRolloutController {
    pub(crate) config: CanaryRolloutConfig,
    pub(crate) status: CanaryRolloutStatus,
    pub(crate) stats: CanaryRolloutStats,
    pub(crate) stable_model_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CanaryDecision {
    pub(crate) use_canary: bool,
    pub(crate) selected_model_id: String,
    pub(crate) canary_eligible: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ModelRolloutSnapshot {
    pub(crate) stable_model_id: String,
    pub(crate) config: CanaryRolloutConfig,
    pub(crate) status: CanaryRolloutStatus,
    pub(crate) stats: CanaryRolloutStats,
}

impl CanaryRolloutController {
    pub(crate) fn new(stable_model_id: String, config: CanaryRolloutConfig) -> Self {
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
