pub mod alerts;
pub mod cost;
pub mod persistence;

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct SystemMetrics {
    chat_completion_success_total: AtomicU64,
    tokens_processed_total: AtomicU64,
    tokens_offloaded_to_scouts_total: AtomicU64,
    verification_fallback_total: AtomicU64,
    output_degeneration_detected_total: AtomicU64,
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
    scout_work_polls_total: AtomicU64,
    scout_work_assignments_total: AtomicU64,
    scout_work_empty_polls_total: AtomicU64,
    scout_work_rate_limited_total: AtomicU64,
    scout_work_overload_reject_total: AtomicU64,
    scout_work_active_cap_reject_total: AtomicU64,
    scout_draft_submissions_total: AtomicU64,
    scout_draft_rate_limited_total: AtomicU64,
    scout_draft_overload_reject_total: AtomicU64,
    scout_draft_reject_missing_identity_total: AtomicU64,
    scout_draft_reject_pow_total: AtomicU64,
    scout_draft_reject_spotcheck_total: AtomicU64,
    scout_draft_reject_empty_tokens_total: AtomicU64,
    scout_draft_duplicates_total: AtomicU64,
    scout_draft_channel_enqueued_total: AtomicU64,
    scout_draft_channel_enqueue_failures_total: AtomicU64,
    speculative_wait_requests_total: AtomicU64,
    speculative_wait_hits_total: AtomicU64,
    speculative_wait_timeouts_total: AtomicU64,
    speculative_wait_mismatched_work_id_total: AtomicU64,
    speculative_verify_attempts_total: AtomicU64,
    speculative_verify_zero_accept_total: AtomicU64,
    scout_client_submit_attempts_total: AtomicU64,
    scout_client_submit_success_total: AtomicU64,
    scout_client_submit_http_failures_total: AtomicU64,
    scout_client_submit_timeouts_total: AtomicU64,
    scout_client_submit_pow_failures_total: AtomicU64,
    scout_client_submit_network_failures_total: AtomicU64,
    scout_client_generate_failures_total: AtomicU64,
    scout_client_fallback_drafts_total: AtomicU64,
    transport_tcp_success_total: AtomicU64,
    transport_tcp_failure_total: AtomicU64,
    transport_websocket_success_total: AtomicU64,
    transport_websocket_failure_total: AtomicU64,
    transport_quic_success_total: AtomicU64,
    transport_quic_failure_total: AtomicU64,
    transport_webrtc_success_total: AtomicU64,
    transport_webrtc_failure_total: AtomicU64,
    transport_relay_success_total: AtomicU64,
    transport_relay_failure_total: AtomicU64,
    speculative_bypass_total: AtomicU64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemMetricsSnapshot {
    pub chat_completion_success_total: u64,
    pub tokens_processed_total: u64,
    pub tokens_offloaded_to_scouts_total: u64,
    pub verification_fallback_total: u64,
    pub output_degeneration_detected_total: u64,
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
    pub scout_work_polls_total: u64,
    pub scout_work_assignments_total: u64,
    pub scout_work_empty_polls_total: u64,
    pub scout_work_rate_limited_total: u64,
    pub scout_work_overload_reject_total: u64,
    pub scout_work_active_cap_reject_total: u64,
    pub scout_draft_submissions_total: u64,
    pub scout_draft_rate_limited_total: u64,
    pub scout_draft_overload_reject_total: u64,
    pub scout_draft_reject_missing_identity_total: u64,
    pub scout_draft_reject_pow_total: u64,
    pub scout_draft_reject_spotcheck_total: u64,
    pub scout_draft_reject_empty_tokens_total: u64,
    pub scout_draft_duplicates_total: u64,
    pub scout_draft_channel_enqueued_total: u64,
    pub scout_draft_channel_enqueue_failures_total: u64,
    pub speculative_wait_requests_total: u64,
    pub speculative_wait_hits_total: u64,
    pub speculative_wait_timeouts_total: u64,
    pub speculative_wait_mismatched_work_id_total: u64,
    pub speculative_verify_attempts_total: u64,
    pub speculative_verify_zero_accept_total: u64,
    pub scout_client_submit_attempts_total: u64,
    pub scout_client_submit_success_total: u64,
    pub scout_client_submit_http_failures_total: u64,
    pub scout_client_submit_timeouts_total: u64,
    pub scout_client_submit_pow_failures_total: u64,
    pub scout_client_submit_network_failures_total: u64,
    pub scout_client_generate_failures_total: u64,
    pub scout_client_fallback_drafts_total: u64,
    pub transport_tcp_success_total: u64,
    pub transport_tcp_failure_total: u64,
    pub transport_websocket_success_total: u64,
    pub transport_websocket_failure_total: u64,
    pub transport_quic_success_total: u64,
    pub transport_quic_failure_total: u64,
    pub transport_webrtc_success_total: u64,
    pub transport_webrtc_failure_total: u64,
    pub transport_relay_success_total: u64,
    pub transport_relay_failure_total: u64,
    pub speculative_bypass_total: u64,
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
    pub fn inc_chat_completion_success(&self) {
        self.chat_completion_success_total
            .fetch_add(1, Ordering::Relaxed);
    }

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

    pub fn inc_output_degeneration_detected(&self) {
        self.output_degeneration_detected_total
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

    pub fn inc_scout_work_poll(&self) {
        self.scout_work_polls_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_work_assignment(&self) {
        self.scout_work_assignments_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_work_empty_poll(&self) {
        self.scout_work_empty_polls_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_work_rate_limited(&self) {
        self.scout_work_rate_limited_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_work_overload_reject(&self) {
        self.scout_work_overload_reject_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_work_active_cap_reject(&self) {
        self.scout_work_active_cap_reject_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_draft_submission(&self) {
        self.scout_draft_submissions_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_draft_rate_limited(&self) {
        self.scout_draft_rate_limited_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_draft_overload_reject(&self) {
        self.scout_draft_overload_reject_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_draft_reject_missing_identity(&self) {
        self.scout_draft_reject_missing_identity_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_draft_reject_pow(&self) {
        self.scout_draft_reject_pow_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_draft_reject_spotcheck(&self) {
        self.scout_draft_reject_spotcheck_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_draft_reject_empty_tokens(&self) {
        self.scout_draft_reject_empty_tokens_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_draft_duplicate(&self) {
        self.scout_draft_duplicates_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_draft_channel_enqueued(&self) {
        self.scout_draft_channel_enqueued_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_draft_channel_enqueue_failure(&self) {
        self.scout_draft_channel_enqueue_failures_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_speculative_wait_request(&self) {
        self.speculative_wait_requests_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_speculative_wait_hit(&self) {
        self.speculative_wait_hits_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_speculative_wait_timeout(&self) {
        self.speculative_wait_timeouts_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_speculative_wait_mismatched_work_id(&self) {
        self.speculative_wait_mismatched_work_id_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_speculative_verify_attempt(&self) {
        self.speculative_verify_attempts_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_speculative_verify_zero_accept(&self) {
        self.speculative_verify_zero_accept_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_client_submit_attempt(&self) {
        self.scout_client_submit_attempts_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_client_submit_success(&self) {
        self.scout_client_submit_success_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_client_submit_http_failure(&self) {
        self.scout_client_submit_http_failures_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_client_submit_timeout(&self) {
        self.scout_client_submit_timeouts_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_client_submit_pow_failure(&self) {
        self.scout_client_submit_pow_failures_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_client_submit_network_failure(&self) {
        self.scout_client_submit_network_failures_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_client_generate_failure(&self) {
        self.scout_client_generate_failures_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_scout_client_fallback_draft(&self) {
        self.scout_client_fallback_drafts_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_transport_tcp_success(&self) {
        self.transport_tcp_success_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_transport_tcp_failure(&self) {
        self.transport_tcp_failure_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_transport_websocket_success(&self) {
        self.transport_websocket_success_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_transport_websocket_failure(&self) {
        self.transport_websocket_failure_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_transport_quic_success(&self) {
        self.transport_quic_success_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_transport_quic_failure(&self) {
        self.transport_quic_failure_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_transport_webrtc_success(&self) {
        self.transport_webrtc_success_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_transport_webrtc_failure(&self) {
        self.transport_webrtc_failure_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_transport_relay_success(&self) {
        self.transport_relay_success_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_transport_relay_failure(&self) {
        self.transport_relay_failure_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_speculative_bypass(&self) {
        self.speculative_bypass_total
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Compute the live speculative draft token acceptance rate.
    /// Returns a value between 0.0 and 1.0.
    /// Returns 1.0 if no verification attempts have been made yet (optimistic)
    /// so that speculative decoding is always tried initially.
    pub fn speculative_acceptance_rate(&self) -> f64 {
        let attempts = self
            .speculative_verify_attempts_total
            .load(Ordering::Relaxed);
        if attempts == 0 {
            return 1.0; // No data yet — be optimistic
        }
        let zero_accepts = self
            .speculative_verify_zero_accept_total
            .load(Ordering::Relaxed);
        // Rate = 1.0 - (zero_accept_verifications / total_verifications)
        // This is the fraction of verification rounds that accepted at least one token.
        let acceptance_round_rate = 1.0 - (zero_accepts as f64 / attempts as f64);
        acceptance_round_rate.clamp(0.0, 1.0)
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

        let mut output = format!(
            concat!(
                "# HELP shard_chat_completion_success_total Successful chat completion responses.\n",
                "# TYPE shard_chat_completion_success_total counter\n",
                "shard_chat_completion_success_total {}\n",
                "# HELP shard_tokens_processed_total Total tokens processed.\n",
                "# TYPE shard_tokens_processed_total counter\n",
                "shard_tokens_processed_total {}\n",
                "# HELP shard_tokens_offloaded_to_scouts_total Total tokens offloaded to scouts.\n",
                "# TYPE shard_tokens_offloaded_to_scouts_total counter\n",
                "shard_tokens_offloaded_to_scouts_total {}\n",
                "# HELP shard_verification_fallback_total Verification fallback executions.\n",
                "# TYPE shard_verification_fallback_total counter\n",
                "shard_verification_fallback_total {}\n",
                "# HELP shard_output_degeneration_detected_total Output degeneration detections that triggered fallback/reset behavior.\n",
                "# TYPE shard_output_degeneration_detected_total counter\n",
                "shard_output_degeneration_detected_total {}\n",
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
            self.chat_completion_success_total.load(Ordering::Relaxed),
            self.tokens_processed_total.load(Ordering::Relaxed),
            self.tokens_offloaded_to_scouts_total.load(Ordering::Relaxed),
            self.verification_fallback_total.load(Ordering::Relaxed),
            self.output_degeneration_detected_total
                .load(Ordering::Relaxed),
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
        );
        output.push_str(&format!(
            concat!(
                "# HELP shard_scout_work_polls_total Scout work polling requests.\n",
                "# TYPE shard_scout_work_polls_total counter\n",
                "shard_scout_work_polls_total {}\n",
                "# HELP shard_scout_work_assignments_total Scout work assignments returned.\n",
                "# TYPE shard_scout_work_assignments_total counter\n",
                "shard_scout_work_assignments_total {}\n",
                "# HELP shard_scout_work_empty_polls_total Scout work polls with empty queue.\n",
                "# TYPE shard_scout_work_empty_polls_total counter\n",
                "shard_scout_work_empty_polls_total {}\n",
                "# HELP shard_scout_draft_submissions_total Draft submissions received.\n",
                "# TYPE shard_scout_draft_submissions_total counter\n",
                "shard_scout_draft_submissions_total {}\n",
                "# HELP shard_scout_draft_reject_missing_identity_total Draft rejects due to missing scout/work identity.\n",
                "# TYPE shard_scout_draft_reject_missing_identity_total counter\n",
                "shard_scout_draft_reject_missing_identity_total {}\n",
                "# HELP shard_scout_draft_reject_pow_total Draft rejects due to failed PoW verification.\n",
                "# TYPE shard_scout_draft_reject_pow_total counter\n",
                "shard_scout_draft_reject_pow_total {}\n",
                "# HELP shard_scout_draft_reject_spotcheck_total Draft rejects due to invalid spot-check proof.\n",
                "# TYPE shard_scout_draft_reject_spotcheck_total counter\n",
                "shard_scout_draft_reject_spotcheck_total {}\n",
                "# HELP shard_scout_draft_reject_empty_tokens_total Draft rejects due to empty/untokenizable draft payload.\n",
                "# TYPE shard_scout_draft_reject_empty_tokens_total counter\n",
                "shard_scout_draft_reject_empty_tokens_total {}\n",
                "# HELP shard_scout_draft_duplicates_total Duplicate draft submissions ignored by idempotency map.\n",
                "# TYPE shard_scout_draft_duplicates_total counter\n",
                "shard_scout_draft_duplicates_total {}\n",
                "# HELP shard_scout_draft_channel_enqueued_total Drafts successfully enqueued to the synchronous verification channel.\n",
                "# TYPE shard_scout_draft_channel_enqueued_total counter\n",
                "shard_scout_draft_channel_enqueued_total {}\n",
                "# HELP shard_scout_draft_channel_enqueue_failures_total Draft channel enqueue failures.\n",
                "# TYPE shard_scout_draft_channel_enqueue_failures_total counter\n",
                "shard_scout_draft_channel_enqueue_failures_total {}\n",
                "# HELP shard_speculative_wait_requests_total Speculative waits started for scout drafts.\n",
                "# TYPE shard_speculative_wait_requests_total counter\n",
                "shard_speculative_wait_requests_total {}\n",
                "# HELP shard_speculative_wait_hits_total Speculative waits that found a matching draft.\n",
                "# TYPE shard_speculative_wait_hits_total counter\n",
                "shard_speculative_wait_hits_total {}\n",
                "# HELP shard_speculative_wait_timeouts_total Speculative waits that timed out.\n",
                "# TYPE shard_speculative_wait_timeouts_total counter\n",
                "shard_speculative_wait_timeouts_total {}\n",
                "# HELP shard_speculative_wait_mismatched_work_id_total Non-matching drafts encountered while waiting for a specific work_id.\n",
                "# TYPE shard_speculative_wait_mismatched_work_id_total counter\n",
                "shard_speculative_wait_mismatched_work_id_total {}\n",
                "# HELP shard_speculative_verify_attempts_total Draft verification attempts executed.\n",
                "# TYPE shard_speculative_verify_attempts_total counter\n",
                "shard_speculative_verify_attempts_total {}\n",
                "# HELP shard_speculative_verify_zero_accept_total Verification attempts that accepted zero draft tokens.\n",
                "# TYPE shard_speculative_verify_zero_accept_total counter\n",
                "shard_speculative_verify_zero_accept_total {}\n",
                "# HELP shard_scout_client_submit_attempts_total Browser scout submit attempts.\n",
                "# TYPE shard_scout_client_submit_attempts_total counter\n",
                "shard_scout_client_submit_attempts_total {}\n",
                "# HELP shard_scout_client_submit_success_total Browser scout successful draft submissions.\n",
                "# TYPE shard_scout_client_submit_success_total counter\n",
                "shard_scout_client_submit_success_total {}\n",
                "# HELP shard_scout_client_submit_http_failures_total Browser scout submission HTTP failures.\n",
                "# TYPE shard_scout_client_submit_http_failures_total counter\n",
                "shard_scout_client_submit_http_failures_total {}\n",
                "# HELP shard_scout_client_submit_timeouts_total Browser scout submission timeouts.\n",
                "# TYPE shard_scout_client_submit_timeouts_total counter\n",
                "shard_scout_client_submit_timeouts_total {}\n",
                "# HELP shard_scout_client_submit_pow_failures_total Browser scout PoW verification failures before submit.\n",
                "# TYPE shard_scout_client_submit_pow_failures_total counter\n",
                "shard_scout_client_submit_pow_failures_total {}\n",
                "# HELP shard_scout_client_submit_network_failures_total Browser scout network/unknown submit failures.\n",
                "# TYPE shard_scout_client_submit_network_failures_total counter\n",
                "shard_scout_client_submit_network_failures_total {}\n",
                "# HELP shard_scout_client_generate_failures_total Browser scout draft-generation failures.\n",
                "# TYPE shard_scout_client_generate_failures_total counter\n",
                "shard_scout_client_generate_failures_total {}\n",
                "# HELP shard_scout_client_fallback_drafts_total Browser scout fallback draft generations used.\n",
                "# TYPE shard_scout_client_fallback_drafts_total counter\n",
                "shard_scout_client_fallback_drafts_total {}\n",
                "# HELP shard_transport_tcp_success_total Successful TCP transport connection establishments.\n",
                "# TYPE shard_transport_tcp_success_total counter\n",
                "shard_transport_tcp_success_total {}\n",
                "# HELP shard_transport_tcp_failure_total Failed TCP transport connection attempts.\n",
                "# TYPE shard_transport_tcp_failure_total counter\n",
                "shard_transport_tcp_failure_total {}\n",
                "# HELP shard_transport_websocket_success_total Successful WebSocket transport connection establishments.\n",
                "# TYPE shard_transport_websocket_success_total counter\n",
                "shard_transport_websocket_success_total {}\n",
                "# HELP shard_transport_websocket_failure_total Failed WebSocket transport connection attempts.\n",
                "# TYPE shard_transport_websocket_failure_total counter\n",
                "shard_transport_websocket_failure_total {}\n",
                "# HELP shard_transport_quic_success_total Successful QUIC transport connection establishments.\n",
                "# TYPE shard_transport_quic_success_total counter\n",
                "shard_transport_quic_success_total {}\n",
                "# HELP shard_transport_quic_failure_total Failed QUIC transport connection attempts.\n",
                "# TYPE shard_transport_quic_failure_total counter\n",
                "shard_transport_quic_failure_total {}\n",
                "# HELP shard_transport_webrtc_success_total Successful WebRTC transport connection establishments.\n",
                "# TYPE shard_transport_webrtc_success_total counter\n",
                "shard_transport_webrtc_success_total {}\n",
                "# HELP shard_transport_webrtc_failure_total Failed WebRTC transport connection attempts.\n",
                "# TYPE shard_transport_webrtc_failure_total counter\n",
                "shard_transport_webrtc_failure_total {}\n",
                "# HELP shard_transport_relay_success_total Successful relay/circuit transport connection establishments.\n",
                "# TYPE shard_transport_relay_success_total counter\n",
                "shard_transport_relay_success_total {}\n",
                "# HELP shard_transport_relay_failure_total Failed relay/circuit transport connection attempts.\n",
                "# TYPE shard_transport_relay_failure_total counter\n",
                "shard_transport_relay_failure_total {}\n",
            ),
            self.scout_work_polls_total.load(Ordering::Relaxed),
            self.scout_work_assignments_total.load(Ordering::Relaxed),
            self.scout_work_empty_polls_total.load(Ordering::Relaxed),
            self.scout_draft_submissions_total.load(Ordering::Relaxed),
            self.scout_draft_reject_missing_identity_total
                .load(Ordering::Relaxed),
            self.scout_draft_reject_pow_total.load(Ordering::Relaxed),
            self.scout_draft_reject_spotcheck_total
                .load(Ordering::Relaxed),
            self.scout_draft_reject_empty_tokens_total
                .load(Ordering::Relaxed),
            self.scout_draft_duplicates_total.load(Ordering::Relaxed),
            self.scout_draft_channel_enqueued_total
                .load(Ordering::Relaxed),
            self.scout_draft_channel_enqueue_failures_total
                .load(Ordering::Relaxed),
            self.speculative_wait_requests_total.load(Ordering::Relaxed),
            self.speculative_wait_hits_total.load(Ordering::Relaxed),
            self.speculative_wait_timeouts_total.load(Ordering::Relaxed),
            self.speculative_wait_mismatched_work_id_total
                .load(Ordering::Relaxed),
            self.speculative_verify_attempts_total.load(Ordering::Relaxed),
            self.speculative_verify_zero_accept_total.load(Ordering::Relaxed),
            self.scout_client_submit_attempts_total
                .load(Ordering::Relaxed),
            self.scout_client_submit_success_total
                .load(Ordering::Relaxed),
            self.scout_client_submit_http_failures_total
                .load(Ordering::Relaxed),
            self.scout_client_submit_timeouts_total
                .load(Ordering::Relaxed),
            self.scout_client_submit_pow_failures_total
                .load(Ordering::Relaxed),
            self.scout_client_submit_network_failures_total
                .load(Ordering::Relaxed),
            self.scout_client_generate_failures_total
                .load(Ordering::Relaxed),
            self.scout_client_fallback_drafts_total
                .load(Ordering::Relaxed),
            self.transport_tcp_success_total.load(Ordering::Relaxed),
            self.transport_tcp_failure_total.load(Ordering::Relaxed),
            self.transport_websocket_success_total
                .load(Ordering::Relaxed),
            self.transport_websocket_failure_total
                .load(Ordering::Relaxed),
            self.transport_quic_success_total.load(Ordering::Relaxed),
            self.transport_quic_failure_total.load(Ordering::Relaxed),
            self.transport_webrtc_success_total.load(Ordering::Relaxed),
            self.transport_webrtc_failure_total.load(Ordering::Relaxed),
            self.transport_relay_success_total.load(Ordering::Relaxed),
            self.transport_relay_failure_total.load(Ordering::Relaxed),
        ));
        output.push_str(&format!(
            concat!(
                "# HELP shard_scout_work_rate_limited_total Scout work polls rejected by per-scout rate limiting.\n",
                "# TYPE shard_scout_work_rate_limited_total counter\n",
                "shard_scout_work_rate_limited_total {}\n",
                "# HELP shard_scout_work_overload_reject_total Scout work polls rejected due to verifier overload/circuit breaker.\n",
                "# TYPE shard_scout_work_overload_reject_total counter\n",
                "shard_scout_work_overload_reject_total {}\n",
                "# HELP shard_scout_work_active_cap_reject_total Scout work polls rejected due to per-verifier active scout cap.\n",
                "# TYPE shard_scout_work_active_cap_reject_total counter\n",
                "shard_scout_work_active_cap_reject_total {}\n",
                "# HELP shard_scout_draft_rate_limited_total Scout draft submissions rejected by per-scout rate limiting.\n",
                "# TYPE shard_scout_draft_rate_limited_total counter\n",
                "shard_scout_draft_rate_limited_total {}\n",
                "# HELP shard_scout_draft_overload_reject_total Scout draft submissions rejected due to verifier overload/circuit breaker.\n",
                "# TYPE shard_scout_draft_overload_reject_total counter\n",
                "shard_scout_draft_overload_reject_total {}\n",
            ),
            self.scout_work_rate_limited_total.load(Ordering::Relaxed),
            self.scout_work_overload_reject_total
                .load(Ordering::Relaxed),
            self.scout_work_active_cap_reject_total
                .load(Ordering::Relaxed),
            self.scout_draft_rate_limited_total
                .load(Ordering::Relaxed),
            self.scout_draft_overload_reject_total
                .load(Ordering::Relaxed),
        ));
        output
    }

    pub fn snapshot(&self) -> SystemMetricsSnapshot {
        SystemMetricsSnapshot {
            chat_completion_success_total: self
                .chat_completion_success_total
                .load(Ordering::Relaxed),
            tokens_processed_total: self.tokens_processed_total.load(Ordering::Relaxed),
            tokens_offloaded_to_scouts_total: self
                .tokens_offloaded_to_scouts_total
                .load(Ordering::Relaxed),
            verification_fallback_total: self.verification_fallback_total.load(Ordering::Relaxed),
            output_degeneration_detected_total: self
                .output_degeneration_detected_total
                .load(Ordering::Relaxed),
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
            scout_work_polls_total: self.scout_work_polls_total.load(Ordering::Relaxed),
            scout_work_assignments_total: self.scout_work_assignments_total.load(Ordering::Relaxed),
            scout_work_empty_polls_total: self.scout_work_empty_polls_total.load(Ordering::Relaxed),
            scout_work_rate_limited_total: self
                .scout_work_rate_limited_total
                .load(Ordering::Relaxed),
            scout_work_overload_reject_total: self
                .scout_work_overload_reject_total
                .load(Ordering::Relaxed),
            scout_work_active_cap_reject_total: self
                .scout_work_active_cap_reject_total
                .load(Ordering::Relaxed),
            scout_draft_submissions_total: self
                .scout_draft_submissions_total
                .load(Ordering::Relaxed),
            scout_draft_rate_limited_total: self
                .scout_draft_rate_limited_total
                .load(Ordering::Relaxed),
            scout_draft_overload_reject_total: self
                .scout_draft_overload_reject_total
                .load(Ordering::Relaxed),
            scout_draft_reject_missing_identity_total: self
                .scout_draft_reject_missing_identity_total
                .load(Ordering::Relaxed),
            scout_draft_reject_pow_total: self.scout_draft_reject_pow_total.load(Ordering::Relaxed),
            scout_draft_reject_spotcheck_total: self
                .scout_draft_reject_spotcheck_total
                .load(Ordering::Relaxed),
            scout_draft_reject_empty_tokens_total: self
                .scout_draft_reject_empty_tokens_total
                .load(Ordering::Relaxed),
            scout_draft_duplicates_total: self.scout_draft_duplicates_total.load(Ordering::Relaxed),
            scout_draft_channel_enqueued_total: self
                .scout_draft_channel_enqueued_total
                .load(Ordering::Relaxed),
            scout_draft_channel_enqueue_failures_total: self
                .scout_draft_channel_enqueue_failures_total
                .load(Ordering::Relaxed),
            speculative_wait_requests_total: self
                .speculative_wait_requests_total
                .load(Ordering::Relaxed),
            speculative_wait_hits_total: self.speculative_wait_hits_total.load(Ordering::Relaxed),
            speculative_wait_timeouts_total: self
                .speculative_wait_timeouts_total
                .load(Ordering::Relaxed),
            speculative_wait_mismatched_work_id_total: self
                .speculative_wait_mismatched_work_id_total
                .load(Ordering::Relaxed),
            speculative_verify_attempts_total: self
                .speculative_verify_attempts_total
                .load(Ordering::Relaxed),
            speculative_verify_zero_accept_total: self
                .speculative_verify_zero_accept_total
                .load(Ordering::Relaxed),
            scout_client_submit_attempts_total: self
                .scout_client_submit_attempts_total
                .load(Ordering::Relaxed),
            scout_client_submit_success_total: self
                .scout_client_submit_success_total
                .load(Ordering::Relaxed),
            scout_client_submit_http_failures_total: self
                .scout_client_submit_http_failures_total
                .load(Ordering::Relaxed),
            scout_client_submit_timeouts_total: self
                .scout_client_submit_timeouts_total
                .load(Ordering::Relaxed),
            scout_client_submit_pow_failures_total: self
                .scout_client_submit_pow_failures_total
                .load(Ordering::Relaxed),
            scout_client_submit_network_failures_total: self
                .scout_client_submit_network_failures_total
                .load(Ordering::Relaxed),
            scout_client_generate_failures_total: self
                .scout_client_generate_failures_total
                .load(Ordering::Relaxed),
            scout_client_fallback_drafts_total: self
                .scout_client_fallback_drafts_total
                .load(Ordering::Relaxed),
            transport_tcp_success_total: self.transport_tcp_success_total.load(Ordering::Relaxed),
            transport_tcp_failure_total: self.transport_tcp_failure_total.load(Ordering::Relaxed),
            transport_websocket_success_total: self
                .transport_websocket_success_total
                .load(Ordering::Relaxed),
            transport_websocket_failure_total: self
                .transport_websocket_failure_total
                .load(Ordering::Relaxed),
            transport_quic_success_total: self.transport_quic_success_total.load(Ordering::Relaxed),
            transport_quic_failure_total: self.transport_quic_failure_total.load(Ordering::Relaxed),
            transport_webrtc_success_total: self
                .transport_webrtc_success_total
                .load(Ordering::Relaxed),
            transport_webrtc_failure_total: self
                .transport_webrtc_failure_total
                .load(Ordering::Relaxed),
            transport_relay_success_total: self
                .transport_relay_success_total
                .load(Ordering::Relaxed),
            transport_relay_failure_total: self
                .transport_relay_failure_total
                .load(Ordering::Relaxed),
            speculative_bypass_total: self.speculative_bypass_total.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SystemMetrics;

    #[test]
    fn snapshot_reflects_counter_updates() {
        let metrics = SystemMetrics::default();
        metrics.inc_chat_completion_success();
        metrics.inc_tokens_processed(12);
        metrics.inc_tokens_offloaded_to_scouts(8);
        metrics.inc_verification_fallback();
        metrics.inc_output_degeneration_detected();
        metrics.inc_task_failures();
        metrics.inc_signature_verification_failures();
        metrics.inc_node_identity_auth_failures();
        metrics.inc_speculative_draft_tokens(10);
        metrics.inc_speculative_accepted_tokens(7);
        metrics.inc_speculative_rejected_tokens(3);
        metrics.inc_scout_work_poll();
        metrics.inc_scout_work_assignment();
        metrics.inc_scout_work_empty_poll();
        metrics.inc_scout_work_rate_limited();
        metrics.inc_scout_work_overload_reject();
        metrics.inc_scout_work_active_cap_reject();
        metrics.inc_scout_draft_submission();
        metrics.inc_scout_draft_rate_limited();
        metrics.inc_scout_draft_overload_reject();
        metrics.inc_scout_draft_reject_missing_identity();
        metrics.inc_scout_draft_reject_pow();
        metrics.inc_scout_draft_reject_spotcheck();
        metrics.inc_scout_draft_reject_empty_tokens();
        metrics.inc_scout_draft_duplicate();
        metrics.inc_scout_draft_channel_enqueued();
        metrics.inc_scout_draft_channel_enqueue_failure();
        metrics.inc_speculative_wait_request();
        metrics.inc_speculative_wait_hit();
        metrics.inc_speculative_wait_timeout();
        metrics.inc_speculative_wait_mismatched_work_id();
        metrics.inc_speculative_verify_attempt();
        metrics.inc_speculative_verify_zero_accept();
        metrics.inc_scout_client_submit_attempt();
        metrics.inc_scout_client_submit_success();
        metrics.inc_scout_client_submit_http_failure();
        metrics.inc_scout_client_submit_timeout();
        metrics.inc_scout_client_submit_pow_failure();
        metrics.inc_scout_client_submit_network_failure();
        metrics.inc_scout_client_generate_failure();
        metrics.inc_scout_client_fallback_draft();
        metrics.inc_transport_tcp_success();
        metrics.inc_transport_tcp_failure();
        metrics.inc_transport_websocket_success();
        metrics.inc_transport_websocket_failure();
        metrics.inc_transport_quic_success();
        metrics.inc_transport_quic_failure();
        metrics.inc_transport_webrtc_success();
        metrics.inc_transport_webrtc_failure();
        metrics.inc_transport_relay_success();
        metrics.inc_transport_relay_failure();

        let snap = metrics.snapshot();
        assert_eq!(snap.chat_completion_success_total, 1);
        assert_eq!(snap.tokens_processed_total, 12);
        assert_eq!(snap.tokens_offloaded_to_scouts_total, 8);
        assert_eq!(snap.verification_fallback_total, 1);
        assert_eq!(snap.output_degeneration_detected_total, 1);
        assert_eq!(snap.task_failures_total, 1);
        assert_eq!(snap.signature_verification_failures_total, 1);
        assert_eq!(snap.node_identity_auth_failures_total, 1);
        assert_eq!(snap.speculative_draft_tokens_total, 10);
        assert_eq!(snap.speculative_accepted_tokens_total, 7);
        assert_eq!(snap.speculative_rejected_tokens_total, 3);
        assert_eq!(snap.scout_work_polls_total, 1);
        assert_eq!(snap.scout_work_assignments_total, 1);
        assert_eq!(snap.scout_work_empty_polls_total, 1);
        assert_eq!(snap.scout_work_rate_limited_total, 1);
        assert_eq!(snap.scout_work_overload_reject_total, 1);
        assert_eq!(snap.scout_work_active_cap_reject_total, 1);
        assert_eq!(snap.scout_draft_submissions_total, 1);
        assert_eq!(snap.scout_draft_rate_limited_total, 1);
        assert_eq!(snap.scout_draft_overload_reject_total, 1);
        assert_eq!(snap.scout_draft_reject_missing_identity_total, 1);
        assert_eq!(snap.scout_draft_reject_pow_total, 1);
        assert_eq!(snap.scout_draft_reject_spotcheck_total, 1);
        assert_eq!(snap.scout_draft_reject_empty_tokens_total, 1);
        assert_eq!(snap.scout_draft_duplicates_total, 1);
        assert_eq!(snap.scout_draft_channel_enqueued_total, 1);
        assert_eq!(snap.scout_draft_channel_enqueue_failures_total, 1);
        assert_eq!(snap.speculative_wait_requests_total, 1);
        assert_eq!(snap.speculative_wait_hits_total, 1);
        assert_eq!(snap.speculative_wait_timeouts_total, 1);
        assert_eq!(snap.speculative_wait_mismatched_work_id_total, 1);
        assert_eq!(snap.speculative_verify_attempts_total, 1);
        assert_eq!(snap.speculative_verify_zero_accept_total, 1);
        assert_eq!(snap.scout_client_submit_attempts_total, 1);
        assert_eq!(snap.scout_client_submit_success_total, 1);
        assert_eq!(snap.scout_client_submit_http_failures_total, 1);
        assert_eq!(snap.scout_client_submit_timeouts_total, 1);
        assert_eq!(snap.scout_client_submit_pow_failures_total, 1);
        assert_eq!(snap.scout_client_submit_network_failures_total, 1);
        assert_eq!(snap.scout_client_generate_failures_total, 1);
        assert_eq!(snap.scout_client_fallback_drafts_total, 1);
        assert_eq!(snap.transport_tcp_success_total, 1);
        assert_eq!(snap.transport_tcp_failure_total, 1);
        assert_eq!(snap.transport_websocket_success_total, 1);
        assert_eq!(snap.transport_websocket_failure_total, 1);
        assert_eq!(snap.transport_quic_success_total, 1);
        assert_eq!(snap.transport_quic_failure_total, 1);
        assert_eq!(snap.transport_webrtc_success_total, 1);
        assert_eq!(snap.transport_webrtc_failure_total, 1);
        assert_eq!(snap.transport_relay_success_total, 1);
        assert_eq!(snap.transport_relay_failure_total, 1);
    }
}
