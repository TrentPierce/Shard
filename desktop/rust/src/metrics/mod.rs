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
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemMetricsSnapshot {
    pub tokens_processed_total: u64,
    pub tokens_offloaded_to_scouts_total: u64,
    pub verification_fallback_total: u64,
    pub task_failures_total: u64,
    pub signature_verification_failures_total: u64,
    pub node_identity_auth_failures_total: u64,
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

    pub fn render_prometheus(
        &self,
        queue_depth: usize,
        active_node_count: usize,
        node_latency_ms: u32,
        scheduler_decision_latency_ms: u64,
        e2e_latency_p50_ms: u64,
        e2e_latency_p95_ms: u64,
        e2e_latency_p99_ms: u64,
        node_uptime_seconds: u64,
    ) -> String {
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
            ),
            self.tokens_processed_total.load(Ordering::Relaxed),
            self.tokens_offloaded_to_scouts_total.load(Ordering::Relaxed),
            self.verification_fallback_total.load(Ordering::Relaxed),
            self.task_failures_total.load(Ordering::Relaxed),
            self.signature_verification_failures_total.load(Ordering::Relaxed),
            self.node_identity_auth_failures_total.load(Ordering::Relaxed),
            queue_depth,
            active_node_count,
            node_latency_ms,
            scheduler_decision_latency_ms,
            e2e_latency_p50_ms,
            e2e_latency_p95_ms,
            e2e_latency_p99_ms,
            node_uptime_seconds,
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
        }
    }
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

        let snap = metrics.snapshot();
        assert_eq!(snap.tokens_processed_total, 12);
        assert_eq!(snap.tokens_offloaded_to_scouts_total, 8);
        assert_eq!(snap.verification_fallback_total, 1);
        assert_eq!(snap.task_failures_total, 1);
        assert_eq!(snap.signature_verification_failures_total, 1);
        assert_eq!(snap.node_identity_auth_failures_total, 1);
    }
}
