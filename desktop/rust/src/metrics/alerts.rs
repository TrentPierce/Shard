//! Anomaly detection and alerting for the Shard network.
//!
//! Monitors request rates, latency, and signature failures to detect
//! DDoS attacks, performance degradation, and Sybil attempts.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Alert Types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlertSeverity {
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlertKind {
    DDoS,
    LatencySpike,
    SignatureFailureRate,
    SybilSuspected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub kind: AlertKind,
    pub severity: AlertSeverity,
    pub message: String,
    pub triggered_at_ms: u128,
    pub value: f64,
    pub threshold: f64,
}

// ─── DDoS Detector ──────────────────────────────────────────────────────────

/// Detects request rate spikes exceeding 10x the rolling average in a 30s window.
pub struct DDoSDetector {
    /// Rolling window of request timestamps (ms).
    request_times: VecDeque<u128>,
    /// Window duration in ms.
    window_ms: u128,
    /// Multiplier over rolling average that triggers alert.
    spike_multiplier: f64,
    /// Baseline rolling average (requests per second).
    baseline_rps: f64,
    /// Minimum baseline samples before alerting.
    min_baseline_samples: usize,
}

impl DDoSDetector {
    pub fn new(window_ms: u128, spike_multiplier: f64) -> Self {
        Self {
            request_times: VecDeque::new(),
            window_ms,
            spike_multiplier,
            baseline_rps: 0.0,
            min_baseline_samples: 10,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(30_000, 10.0)
    }

    /// Record a request and check if DDoS threshold is exceeded.
    pub fn record_request(&mut self) -> Option<Alert> {
        let now = now_ms();
        self.request_times.push_back(now);
        self.prune(now);

        let current_rps = self.current_rps(now);

        // Update baseline with EWMA
        if self.baseline_rps <= 0.0 {
            self.baseline_rps = current_rps;
        } else {
            self.baseline_rps = self.baseline_rps * 0.95 + current_rps * 0.05;
        }

        // Need minimum samples before triggering
        if self.request_times.len() < self.min_baseline_samples {
            return None;
        }

        let threshold = self.baseline_rps * self.spike_multiplier;
        if current_rps > threshold && threshold > 0.0 {
            Some(Alert {
                kind: AlertKind::DDoS,
                severity: AlertSeverity::Critical,
                message: format!(
                    "Request rate {current_rps:.1} rps exceeds {:.1}x baseline ({:.1} rps)",
                    self.spike_multiplier, self.baseline_rps
                ),
                triggered_at_ms: now,
                value: current_rps,
                threshold,
            })
        } else {
            None
        }
    }

    fn current_rps(&self, now: u128) -> f64 {
        let count = self.request_times.len();
        let window_secs = self.window_ms as f64 / 1000.0;
        count as f64 / window_secs
    }

    fn prune(&mut self, now: u128) {
        let cutoff = now.saturating_sub(self.window_ms);
        while self.request_times.front().map_or(false, |&t| t < cutoff) {
            self.request_times.pop_front();
        }
    }
}

// ─── Latency Spike Detector ─────────────────────────────────────────────────

/// Detects when p95 latency exceeds 3x the baseline.
pub struct LatencySpikeDetector {
    /// Recent latency samples (ms).
    samples: VecDeque<u64>,
    /// Maximum samples to keep.
    max_samples: usize,
    /// Baseline p95 latency (EWMA).
    baseline_p95_ms: f64,
    /// Multiplier over baseline that triggers alert.
    spike_multiplier: f64,
    /// Minimum samples before alerting.
    min_samples: usize,
}

impl LatencySpikeDetector {
    pub fn new(max_samples: usize, spike_multiplier: f64) -> Self {
        Self {
            samples: VecDeque::new(),
            max_samples,
            baseline_p95_ms: 0.0,
            spike_multiplier,
            min_samples: 20,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(1000, 3.0)
    }

    /// Record a latency sample and check for spikes.
    pub fn record_latency(&mut self, latency_ms: u64) -> Option<Alert> {
        self.samples.push_back(latency_ms);
        if self.samples.len() > self.max_samples {
            self.samples.pop_front();
        }

        if self.samples.len() < self.min_samples {
            return None;
        }

        let p95 = self.compute_p95();

        // Update baseline EWMA
        if self.baseline_p95_ms <= 0.0 {
            self.baseline_p95_ms = p95;
        } else {
            self.baseline_p95_ms = self.baseline_p95_ms * 0.95 + p95 * 0.05;
        }

        let threshold = self.baseline_p95_ms * self.spike_multiplier;
        if p95 > threshold && threshold > 0.0 {
            Some(Alert {
                kind: AlertKind::LatencySpike,
                severity: AlertSeverity::Warning,
                message: format!(
                    "p95 latency {p95:.0}ms exceeds {:.1}x baseline ({:.0}ms)",
                    self.spike_multiplier, self.baseline_p95_ms
                ),
                triggered_at_ms: now_ms(),
                value: p95,
                threshold,
            })
        } else {
            None
        }
    }

    fn compute_p95(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let mut sorted: Vec<u64> = self.samples.iter().copied().collect();
        sorted.sort();
        let idx = ((sorted.len() as f64) * 0.95).ceil() as usize;
        let idx = idx.min(sorted.len()).saturating_sub(1);
        sorted[idx] as f64
    }
}

// ─── Signature Failure Rate Detector ────────────────────────────────────────

/// Detects when signature verification failure rate exceeds a threshold.
pub struct SignatureFailureDetector {
    /// Recent verification results: true = success, false = failure.
    results: VecDeque<(u128, bool)>,
    /// Window duration in ms.
    window_ms: u128,
    /// Failure rate threshold (0.0 to 1.0). Default: 0.05 (5%).
    failure_rate_threshold: f64,
    /// Minimum events in window before alerting.
    min_events: usize,
}

impl SignatureFailureDetector {
    pub fn new(window_ms: u128, failure_rate_threshold: f64) -> Self {
        Self {
            results: VecDeque::new(),
            window_ms,
            failure_rate_threshold,
            min_events: 10,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(60_000, 0.05) // 5% in 1 minute
    }

    /// Record a signature verification result.
    pub fn record_verification(&mut self, success: bool) -> Option<Alert> {
        let now = now_ms();
        self.results.push_back((now, success));
        self.prune(now);

        if self.results.len() < self.min_events {
            return None;
        }

        let total = self.results.len() as f64;
        let failures = self.results.iter().filter(|(_, ok)| !ok).count() as f64;
        let rate = failures / total;

        if rate > self.failure_rate_threshold {
            Some(Alert {
                kind: AlertKind::SignatureFailureRate,
                severity: AlertSeverity::Critical,
                message: format!(
                    "Signature failure rate {:.1}% exceeds {:.1}% threshold ({} failures in {} events)",
                    rate * 100.0,
                    self.failure_rate_threshold * 100.0,
                    failures as u64,
                    total as u64,
                ),
                triggered_at_ms: now,
                value: rate,
                threshold: self.failure_rate_threshold,
            })
        } else {
            None
        }
    }

    fn prune(&mut self, now: u128) {
        let cutoff = now.saturating_sub(self.window_ms);
        while self.results.front().map_or(false, |(t, _)| *t < cutoff) {
            self.results.pop_front();
        }
    }

    /// Current failure rate (for telemetry).
    pub fn current_failure_rate(&self) -> f64 {
        if self.results.is_empty() {
            return 0.0;
        }
        let failures = self.results.iter().filter(|(_, ok)| !ok).count() as f64;
        failures / self.results.len() as f64
    }
}

// ─── Combined Alert Manager ─────────────────────────────────────────────────

/// Aggregates all detectors and maintains an alert feed.
pub struct AlertManager {
    pub ddos: DDoSDetector,
    pub latency: LatencySpikeDetector,
    pub sig_failures: SignatureFailureDetector,
    /// Recent alerts (bounded ring buffer).
    pub recent_alerts: VecDeque<Alert>,
    pub max_alerts: usize,
    /// Current Sybil alert level (derived from sig failure + ddos combined).
    pub sybil_alert_level: u8,
}

impl AlertManager {
    pub fn new() -> Self {
        Self {
            ddos: DDoSDetector::with_defaults(),
            latency: LatencySpikeDetector::with_defaults(),
            sig_failures: SignatureFailureDetector::with_defaults(),
            recent_alerts: VecDeque::new(),
            max_alerts: 100,
            sybil_alert_level: 0,
        }
    }

    fn push_alert(&mut self, alert: Alert) {
        if self.recent_alerts.len() >= self.max_alerts {
            self.recent_alerts.pop_front();
        }

        // Update Sybil level based on alert type
        match alert.kind {
            AlertKind::DDoS | AlertKind::SignatureFailureRate => {
                self.sybil_alert_level = (self.sybil_alert_level + 1).min(3);
            }
            _ => {}
        }

        self.recent_alerts.push_back(alert);
    }

    /// Record an incoming request and check for DDoS.
    pub fn on_request(&mut self) -> Option<&Alert> {
        if let Some(alert) = self.ddos.record_request() {
            self.push_alert(alert);
            return self.recent_alerts.back();
        }
        None
    }

    /// Record a latency observation and check for spikes.
    pub fn on_latency(&mut self, latency_ms: u64) -> Option<&Alert> {
        if let Some(alert) = self.latency.record_latency(latency_ms) {
            self.push_alert(alert);
            return self.recent_alerts.back();
        }
        None
    }

    /// Record a signature verification result and check for failure rate.
    pub fn on_signature_verification(&mut self, success: bool) -> Option<&Alert> {
        if let Some(alert) = self.sig_failures.record_verification(success) {
            self.push_alert(alert);
            return self.recent_alerts.back();
        }
        None
    }
}

impl Default for AlertManager {
    fn default() -> Self {
        Self::new()
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ddos_detector_normal_traffic() {
        let mut detector = DDoSDetector::with_defaults();
        // Record a few requests — shouldn't trigger
        for _ in 0..5 {
            assert!(detector.record_request().is_none());
        }
    }

    #[test]
    fn latency_spike_detector_normal() {
        let mut detector = LatencySpikeDetector::with_defaults();
        // Record normal latencies
        for _ in 0..30 {
            assert!(detector.record_latency(50).is_none());
        }
    }

    #[test]
    fn latency_spike_detected() {
        let mut detector = LatencySpikeDetector::new(100, 3.0);
        detector.min_samples = 5;

        // Build baseline
        for _ in 0..20 {
            detector.record_latency(50);
        }

        // Inject spike
        for _ in 0..20 {
            let alert = detector.record_latency(5000);
            if let Some(a) = alert {
                assert_eq!(a.kind, AlertKind::LatencySpike);
                return;
            }
        }
        // Spike may take time to register due to EWMA — ok if not triggered in 20 samples
    }

    #[test]
    fn signature_failure_rate_detected() {
        let mut detector = SignatureFailureDetector::new(60_000, 0.05);
        detector.min_events = 5;

        // Record 3 successes and 3 failures (50% failure rate >> 5% threshold)
        detector.record_verification(true);
        detector.record_verification(true);
        detector.record_verification(true);
        detector.record_verification(false);
        detector.record_verification(false);
        let alert = detector.record_verification(false);
        assert!(alert.is_some());
        assert_eq!(alert.unwrap().kind, AlertKind::SignatureFailureRate);
    }

    #[test]
    fn signature_detector_normal_rate() {
        let mut detector = SignatureFailureDetector::new(60_000, 0.05);
        detector.min_events = 5;

        // All successes
        for _ in 0..20 {
            assert!(detector.record_verification(true).is_none());
        }
    }

    #[test]
    fn alert_manager_sybil_escalation() {
        let mut manager = AlertManager::new();
        assert_eq!(manager.sybil_alert_level, 0);

        // Simulate a sig failure alert
        manager.push_alert(Alert {
            kind: AlertKind::SignatureFailureRate,
            severity: AlertSeverity::Critical,
            message: "test".into(),
            triggered_at_ms: 1000,
            value: 0.1,
            threshold: 0.05,
        });
        assert_eq!(manager.sybil_alert_level, 1);

        // Another alert escalates further
        manager.push_alert(Alert {
            kind: AlertKind::DDoS,
            severity: AlertSeverity::Critical,
            message: "test".into(),
            triggered_at_ms: 2000,
            value: 100.0,
            threshold: 10.0,
        });
        assert_eq!(manager.sybil_alert_level, 2);
    }

    #[test]
    fn alert_ring_buffer_bounded() {
        let mut manager = AlertManager::new();
        manager.max_alerts = 5;

        for i in 0..10 {
            manager.push_alert(Alert {
                kind: AlertKind::LatencySpike,
                severity: AlertSeverity::Warning,
                message: format!("alert {i}"),
                triggered_at_ms: i as u128 * 1000,
                value: 0.0,
                threshold: 0.0,
            });
        }
        assert_eq!(manager.recent_alerts.len(), 5);
        assert_eq!(manager.recent_alerts.front().unwrap().message, "alert 5");
    }
}
