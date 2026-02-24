pub mod alerts;
pub mod cost;
pub mod persistence;

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct SystemMetrics {
    tokens_processed_total: AtomicU64,
    tokens_offloaded_to_scouts_total: AtomicU64,
    verification_fallback_total: AtomicU64,
    task_failures_total: AtomicU64,
    signature_verification_failures_total: AtomicU64,
    node_identity_auth_failures_total: AtomicU64,
    scout_dropoff_total: AtomicU64,
    pow_challenges_issued_total: AtomicU64,
    pow_challenges_failed_total: AtomicU64,
    private_route_total: AtomicU64,
    prompt_replay_total: AtomicU64,
    fallback_invocations_total: AtomicU64,
    speculative_draft_tokens_total: AtomicU64,
    speculative_accepted_tokens_total: AtomicU64,
    speculative_rejected_tokens_total: AtomicU64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemMetricsSnapshot {
    pub tokens_processed_total: u64,
    pub tokens_offloaded_to_scouts_total: u64,
    pub verification_fallback_total: u64,
    pub task_failures_total: u64,
    pub signature_verification_failures_total: u64,
    pub node_identity_auth_failures_total: u64,
    pub scout_dropoff_total: u64,
    pub pow_challenges_issued_total: u64,
    pub pow_challenges_failed_total: u64,
    pub private_route_total: u64,
    pub prompt_replay_total: u64,
    pub fallback_invocations_total: u64,
    pub speculative_draft_tokens_total: u64,
    pub speculative_accepted_tokens_total: u64,
    pub speculative_rejected_tokens_total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetricReport {
    pub node_pubkey: String,
    pub role: String,
    pub queue_depth: u64,
    pub node_latency_ms: u64,
    pub uptime_seconds: u64,
    #[serde(default)]
    pub timestamp_ms: Option<u128>,
}

#[derive(Debug, Clone, Copy)]
pub struct PrometheusSample {
    pub queue_depth: usize,
    pub active_node_count: usize,
    pub node_latency_ms: u32,
    pub scheduler_decision_latency_ms: u64,
    pub e2e_latency_p50_ms: u64,
    pub e2e_latency_p95_ms: u64,
    pub e2e_latency_p99_ms: u64,
    pub node_uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeMetricSnapshot {
    pub node_pubkey: String,
    pub role: String,
    pub queue_depth: u64,
    pub node_latency_ms: u64,
    pub uptime_seconds: u64,
    pub last_report_ms: u128,
    pub healthy: bool,
}

impl SystemMetrics {
    pub fn inc_tokens_processed(&self, value: u64) {
        self.tokens_processed_total
            .fetch_add(value, Ordering::Relaxed);
    }

    pub fn inc_tokens_offloaded_to_scouts(&self, value: u64) {
        self.tokens_offloaded_to_scouts_total
            .fetch_add(value, Ordering::Relaxed);
    }

    pub fn inc_verification_fallback(&self) {
        self.verification_fallback_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_task_failures(&self) {
        self.task_failures_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_signature_verification_failures(&self) {
        self.signature_verification_failures_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_node_identity_auth_failures(&self) {
        self.node_identity_auth_failures_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_dropoff(&self) {
        self.scout_dropoff_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_pow_challenges_issued(&self) {
        self.pow_challenges_issued_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_pow_challenges_failed(&self) {
        self.pow_challenges_failed_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_private_route(&self) {
        self.private_route_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_prompt_replay(&self) {
        self.prompt_replay_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_fallback_invocations(&self) {
        self.fallback_invocations_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_speculative_draft_tokens(&self, value: u64) {
        self.speculative_draft_tokens_total
            .fetch_add(value, Ordering::Relaxed);
    }

    pub fn inc_speculative_accepted_tokens(&self, value: u64) {
        self.speculative_accepted_tokens_total
            .fetch_add(value, Ordering::Relaxed);
    }

    pub fn inc_speculative_rejected_tokens(&self, value: u64) {
        self.speculative_rejected_tokens_total
            .fetch_add(value, Ordering::Relaxed);
    }

    pub fn render_prometheus(&self, sample: PrometheusSample) -> String {
        let draft_total = self.speculative_draft_tokens_total.load(Ordering::Relaxed);
        let accepted_total = self
            .speculative_accepted_tokens_total
            .load(Ordering::Relaxed);
        let rejected_total = self
            .speculative_rejected_tokens_total
            .load(Ordering::Relaxed);
        let acceptance_rate = if draft_total == 0 {
            0.0
        } else {
            accepted_total as f64 / draft_total as f64
        };
        let reject_rate = if draft_total == 0 {
            0.0
        } else {
            rejected_total as f64 / draft_total as f64
        };
        let speedup_ratio = if accepted_total == 0 {
            1.0
        } else {
            1.0 + (accepted_total as f64 / (accepted_total + 1) as f64)
        };

        format!(
            concat!(
                "# HELP shard_tokens_processed_total Total tokens processed.\n",
                "# TYPE shard_tokens_processed_total counter\n",
                "shard_tokens_processed_total {}\n",
                "# HELP shard_tokens_offloaded_to_scouts_total Total tokens offloaded to scouts.\n",
                "# TYPE shard_tokens_offloaded_to_scouts_total counter\n",
                "shard_tokens_offloaded_to_scouts_total {}\n",
                "# HELP shard_verification_fallback_total Verification fallback executions.\n",
                "# TYPE shard_verification_fallback_total counter\n",
                "shard_verification_fallback_total {}\n",
                "# HELP shard_task_failures_total Total task failures.\n",
                "# TYPE shard_task_failures_total counter\n",
                "shard_task_failures_total {}\n",
                "# HELP shard_signature_verification_failures_total Signature verification failures.\n",
                "# TYPE shard_signature_verification_failures_total counter\n",
                "shard_signature_verification_failures_total {}\n",
                "# HELP shard_node_identity_auth_failures_total Node identity authentication failures.\n",
                "# TYPE shard_node_identity_auth_failures_total counter\n",
                "shard_node_identity_auth_failures_total {}\n",
                "# HELP shard_queue_depth Current work queue depth.\n",
                "# TYPE shard_queue_depth gauge\n",
                "shard_queue_depth {}\n",
                "# HELP shard_active_node_count Current active node count.\n",
                "# TYPE shard_active_node_count gauge\n",
                "shard_active_node_count {}\n",
                "# HELP shard_node_latency_ms Current average node latency in ms.\n",
                "# TYPE shard_node_latency_ms gauge\n",
                "shard_node_latency_ms {}\n",
                "# HELP shard_scheduler_decision_latency_ms Scheduler decision latency in ms.\n",
                "# TYPE shard_scheduler_decision_latency_ms gauge\n",
                "shard_scheduler_decision_latency_ms {}\n",
                "# HELP shard_e2e_request_latency_p50_ms End-to-end request latency p50 in ms.\n",
                "# TYPE shard_e2e_request_latency_p50_ms gauge\n",
                "shard_e2e_request_latency_p50_ms {}\n",
                "# HELP shard_e2e_request_latency_p95_ms End-to-end request latency p95 in ms.\n",
                "# TYPE shard_e2e_request_latency_p95_ms gauge\n",
                "shard_e2e_request_latency_p95_ms {}\n",
                "# HELP shard_e2e_request_latency_p99_ms End-to-end request latency p99 in ms.\n",
                "# TYPE shard_e2e_request_latency_p99_ms gauge\n",
                "shard_e2e_request_latency_p99_ms {}\n",
                "# HELP shard_node_uptime_seconds Node uptime in seconds.\n",
                "# TYPE shard_node_uptime_seconds gauge\n",
                "shard_node_uptime_seconds {}\n",
                "# HELP shard_scout_dropoff_total Scout disconnection events.\n",
                "# TYPE shard_scout_dropoff_total counter\n",
                "shard_scout_dropoff_total {}\n",
                "# HELP shard_pow_challenges_issued_total PoW challenges issued.\n",
                "# TYPE shard_pow_challenges_issued_total counter\n",
                "shard_pow_challenges_issued_total {}\n",
                "# HELP shard_pow_challenges_failed_total PoW challenges failed.\n",
                "# TYPE shard_pow_challenges_failed_total counter\n",
                "shard_pow_challenges_failed_total {}\n",
                "# HELP shard_private_route_total Private route requests.\n",
                "# TYPE shard_private_route_total counter\n",
                "shard_private_route_total {}\n",
                "# HELP shard_prompt_replay_total Prompt replay fallback events.\n",
                "# TYPE shard_prompt_replay_total counter\n",
                "shard_prompt_replay_total {}\n",
                "# HELP shard_fallback_invocations_total Centralized fallback invocations.\n",
                "# TYPE shard_fallback_invocations_total counter\n",
                "shard_fallback_invocations_total {}\n",
                "# HELP shard_speculative_draft_tokens_total Draft tokens received for speculative verification.\n",
                "# TYPE shard_speculative_draft_tokens_total counter\n",
                "shard_speculative_draft_tokens_total {}\n",
                "# HELP shard_speculative_accepted_tokens_total Draft tokens accepted by verifier.\n",
                "# TYPE shard_speculative_accepted_tokens_total counter\n",
                "shard_speculative_accepted_tokens_total {}\n",
                "# HELP shard_speculative_rejected_tokens_total Draft tokens rejected by verifier.\n",
                "# TYPE shard_speculative_rejected_tokens_total counter\n",
                "shard_speculative_rejected_tokens_total {}\n",
                "# HELP shard_speculative_acceptance_rate Ratio of accepted speculative draft tokens.\n",
                "# TYPE shard_speculative_acceptance_rate gauge\n",
                "shard_speculative_acceptance_rate {:.6}\n",
                "# HELP shard_speculative_reject_rate Ratio of rejected speculative draft tokens.\n",
                "# TYPE shard_speculative_reject_rate gauge\n",
                "shard_speculative_reject_rate {:.6}\n",
                "# HELP shard_speculative_speedup_ratio Estimated speculative speedup ratio.\n",
                "# TYPE shard_speculative_speedup_ratio gauge\n",
                "shard_speculative_speedup_ratio {:.6}\n",
            ),
            self.tokens_processed_total.load(Ordering::Relaxed),
            self.tokens_offloaded_to_scouts_total.load(Ordering::Relaxed),
            self.verification_fallback_total.load(Ordering::Relaxed),
            self.task_failures_total.load(Ordering::Relaxed),
            self.signature_verification_failures_total
                .load(Ordering::Relaxed),
            self.node_identity_auth_failures_total.load(Ordering::Relaxed),
            sample.queue_depth,
            sample.active_node_count,
            sample.node_latency_ms,
            sample.scheduler_decision_latency_ms,
            sample.e2e_latency_p50_ms,
            sample.e2e_latency_p95_ms,
            sample.e2e_latency_p99_ms,
            sample.node_uptime_seconds,
            self.scout_dropoff_total.load(Ordering::Relaxed),
            self.pow_challenges_issued_total.load(Ordering::Relaxed),
            self.pow_challenges_failed_total.load(Ordering::Relaxed),
            self.private_route_total.load(Ordering::Relaxed),
            self.prompt_replay_total.load(Ordering::Relaxed),
            self.fallback_invocations_total.load(Ordering::Relaxed),
            draft_total,
            accepted_total,
            rejected_total,
            acceptance_rate,
            reject_rate,
            speedup_ratio,
        )
    }

    pub fn snapshot(&self) -> SystemMetricsSnapshot {
        SystemMetricsSnapshot {
            tokens_processed_total: self.tokens_processed_total.load(Ordering::Relaxed),
            tokens_offloaded_to_scouts_total: self
                .tokens_offloaded_to_scouts_total
                .load(Ordering::Relaxed),
            verification_fallback_total: self.verification_fallback_total.load(Ordering::Relaxed),
            task_failures_total: self.task_failures_total.load(Ordering::Relaxed),
            signature_verification_failures_total: self
                .signature_verification_failures_total
                .load(Ordering::Relaxed),
            node_identity_auth_failures_total: self
                .node_identity_auth_failures_total
                .load(Ordering::Relaxed),
            scout_dropoff_total: self.scout_dropoff_total.load(Ordering::Relaxed),
            pow_challenges_issued_total: self.pow_challenges_issued_total.load(Ordering::Relaxed),
            pow_challenges_failed_total: self.pow_challenges_failed_total.load(Ordering::Relaxed),
            private_route_total: self.private_route_total.load(Ordering::Relaxed),
            prompt_replay_total: self.prompt_replay_total.load(Ordering::Relaxed),
            fallback_invocations_total: self.fallback_invocations_total.load(Ordering::Relaxed),
            speculative_draft_tokens_total: self
                .speculative_draft_tokens_total
                .load(Ordering::Relaxed),
            speculative_accepted_tokens_total: self
                .speculative_accepted_tokens_total
                .load(Ordering::Relaxed),
            speculative_rejected_tokens_total: self
                .speculative_rejected_tokens_total
                .load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SystemMetrics;

    #[test]
    fn snapshot_reflects_counter_updates() {
        let metrics = SystemMetrics::default();
        metrics.inc_tokens_processed(12);
        metrics.inc_tokens_offloaded_to_scouts(8);
        metrics.inc_verification_fallback();
        metrics.inc_task_failures();
        metrics.inc_signature_verification_failures();
        metrics.inc_node_identity_auth_failures();
        metrics.inc_speculative_draft_tokens(10);
        metrics.inc_speculative_accepted_tokens(7);
        metrics.inc_speculative_rejected_tokens(3);

        let snap = metrics.snapshot();
        assert_eq!(snap.tokens_processed_total, 12);
        assert_eq!(snap.tokens_offloaded_to_scouts_total, 8);
        assert_eq!(snap.verification_fallback_total, 1);
        assert_eq!(snap.task_failures_total, 1);
        assert_eq!(snap.signature_verification_failures_total, 1);
        assert_eq!(snap.node_identity_auth_failures_total, 1);
        assert_eq!(snap.speculative_draft_tokens_total, 10);
        assert_eq!(snap.speculative_accepted_tokens_total, 7);
        assert_eq!(snap.speculative_rejected_tokens_total, 3);
    }
}
