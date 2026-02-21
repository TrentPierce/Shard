//! Centralized API fallback and prompt-replay logic.
//!
//! When a Scout disconnects mid-generation or times out, this module handles
//! seamless fallback to centralized inference or re-dispatch to a new Scout.
//!
//! Routing logic:
//! 1. If `input_token_count > LONG_CONTEXT_THRESHOLD` → always centralized fallback
//! 2. If short prompt AND <25% complete → re-dispatch to new Scout
//! 3. If short prompt AND ≥25% complete → centralized fallback with prefix

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Default threshold above which prompts always route to centralized API.
pub const DEFAULT_LONG_CONTEXT_THRESHOLD: usize = 1024;

/// Default completion percentage below which short prompts are re-dispatched.
pub const RE_DISPATCH_THRESHOLD_PERCENT: f32 = 0.25;

// ─── Types ──────────────────────────────────────────────────────────────────

/// Describes what action to take when a Scout drops mid-generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FallbackAction {
    /// Re-dispatch the full prompt to a new Scout (short prompt, early failure).
    ReDispatchToScout,
    /// Fall back to centralized API, appending already-generated tokens.
    CentralizedFallback,
}

/// The reason for choosing a particular fallback route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FallbackReason {
    /// Input prompt exceeds the long-context threshold.
    LongContext,
    /// Short prompt, but generation was ≥25% complete.
    CompletionThreshold,
    /// Short prompt, generation was <25% complete, so re-dispatch is cheap.
    EarlyFailure,
    /// Explicit race timeout (no Scout responded in time).
    RaceTimeout,
    /// Privacy routing — prompt marked as sensitive.
    PrivateRoute,
}

/// Tracks the state of an active generation request for fallback decisions.
#[derive(Debug, Clone, Serialize)]
pub struct ActiveRequestState {
    pub request_id: String,
    pub input_token_count: usize,
    pub expected_output_tokens: usize,
    pub tokens_generated_so_far: usize,
    pub prompt_context: String,
    pub generated_tokens: Vec<String>,
    pub started_at_ms: u128,
    pub scout_peer_id: Option<String>,
}

impl ActiveRequestState {
    pub fn completion_ratio(&self) -> f32 {
        if self.expected_output_tokens == 0 {
            return 1.0;
        }
        self.tokens_generated_so_far as f32 / self.expected_output_tokens as f32
    }
}

/// Signed event logged when a fallback occurs, for auditability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptReplayEvent {
    pub request_id: String,
    pub action: FallbackAction,
    pub reason: FallbackReason,
    pub input_token_count: usize,
    pub tokens_generated_so_far: usize,
    pub completion_ratio: f32,
    pub original_scout_peer_id: Option<String>,
    pub timestamp_ms: u128,
}

// ─── Fallback Configuration ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FallbackConfig {
    /// Prompts with more tokens than this always go to centralized API.
    pub long_context_threshold: usize,
    /// Completion ratio below which short prompts are re-dispatched.
    pub re_dispatch_threshold: f32,
    /// URL of the centralized fallback API endpoint.
    pub fallback_api_url: String,
}

impl FallbackConfig {
    pub fn from_env() -> Self {
        let threshold = std::env::var("LONG_CONTEXT_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_LONG_CONTEXT_THRESHOLD);

        let fallback_url = std::env::var("FALLBACK_API_URL").unwrap_or_else(|_| {
            "https://api.ourcompany.com/v1/internal/scout-fallback".to_string()
        });

        Self {
            long_context_threshold: threshold,
            re_dispatch_threshold: RE_DISPATCH_THRESHOLD_PERCENT,
            fallback_api_url: fallback_url,
        }
    }
}

// ─── Decision Logic ─────────────────────────────────────────────────────────

/// Determine the fallback action when a Scout drops mid-generation.
pub fn decide_fallback(
    request: &ActiveRequestState,
    config: &FallbackConfig,
) -> (FallbackAction, FallbackReason) {
    // Rule 1: Long prompts always go centralized
    if request.input_token_count > config.long_context_threshold {
        return (
            FallbackAction::CentralizedFallback,
            FallbackReason::LongContext,
        );
    }

    // Rule 2/3: Short prompts — check completion ratio
    let ratio = request.completion_ratio();
    if ratio < config.re_dispatch_threshold {
        (
            FallbackAction::ReDispatchToScout,
            FallbackReason::EarlyFailure,
        )
    } else {
        (
            FallbackAction::CentralizedFallback,
            FallbackReason::CompletionThreshold,
        )
    }
}

/// Build the prompt to send to the centralized API, appending generated tokens.
pub fn build_continuation_prompt(request: &ActiveRequestState) -> String {
    if request.generated_tokens.is_empty() {
        return request.prompt_context.clone();
    }
    format!(
        "{}{}",
        request.prompt_context,
        request.generated_tokens.join("")
    )
}

/// Create a signed prompt replay event for the audit trail.
pub fn create_replay_event(
    request: &ActiveRequestState,
    action: FallbackAction,
    reason: FallbackReason,
) -> PromptReplayEvent {
    PromptReplayEvent {
        request_id: request.request_id.clone(),
        action,
        reason,
        input_token_count: request.input_token_count,
        tokens_generated_so_far: request.tokens_generated_so_far,
        completion_ratio: request.completion_ratio(),
        original_scout_peer_id: request.scout_peer_id.clone(),
        timestamp_ms: now_ms(),
    }
}

/// Execute the centralized fallback by calling the fallback API.
/// Returns the response body as a string (caller handles streaming).
pub async fn execute_centralized_fallback(
    config: &FallbackConfig,
    request: &ActiveRequestState,
) -> Result<String, String> {
    tracing::info!(
        request_id = %request.request_id,
        tokens = request.input_token_count,
        "triggering centralized fallback"
    );
    let client = reqwest::Client::new();
    let prompt = build_continuation_prompt(request);

    let body = serde_json::json!({
        "model": "default",
        "messages": [{"role": "user", "content": prompt}],
        "stream": false,
        "request_id": request.request_id,
    });

    let response = client
        .post(&config.fallback_api_url)
        .header("Content-Type", "application/json")
        .json(&body)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| {
            let err = format!("fallback API request failed: {e}");
            tracing::error!(request_id = %request.request_id, %err);
            err
        })?;

    if !response.status().is_success() {
        let err = format!("fallback API returned status {}", response.status());
        tracing::error!(request_id = %request.request_id, %err);
        return Err(err);
    }

    let text = response.text().await.map_err(|e| {
        let err = format!("failed to read fallback API response: {e}");
        tracing::error!(request_id = %request.request_id, %err);
        err
    })?;

    tracing::info!(request_id = %request.request_id, "centralized fallback successful");
    Ok(text)
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

    fn make_request(input_tokens: usize, generated: usize, expected: usize) -> ActiveRequestState {
        ActiveRequestState {
            request_id: "test-req-1".to_string(),
            input_token_count: input_tokens,
            expected_output_tokens: expected,
            tokens_generated_so_far: generated,
            prompt_context: "Hello world".to_string(),
            generated_tokens: (0..generated).map(|i| format!(" tok{i}")).collect(),
            started_at_ms: 1000,
            scout_peer_id: Some("peer-abc".to_string()),
        }
    }

    fn default_config() -> FallbackConfig {
        FallbackConfig {
            long_context_threshold: 1024,
            re_dispatch_threshold: 0.25,
            fallback_api_url: "http://localhost:8080/fallback".to_string(),
        }
    }

    #[test]
    fn long_context_always_centralizes() {
        let request = make_request(2000, 10, 100); // 2000 input tokens
        let config = default_config();
        let (action, reason) = decide_fallback(&request, &config);
        assert_eq!(action, FallbackAction::CentralizedFallback);
        assert_eq!(reason, FallbackReason::LongContext);
    }

    #[test]
    fn long_context_even_at_zero_completion() {
        let request = make_request(6000, 0, 100);
        let config = default_config();
        let (action, reason) = decide_fallback(&request, &config);
        assert_eq!(action, FallbackAction::CentralizedFallback);
        assert_eq!(reason, FallbackReason::LongContext);
    }

    #[test]
    fn short_prompt_early_failure_re_dispatches() {
        let request = make_request(50, 5, 100); // 5% completion
        let config = default_config();
        let (action, reason) = decide_fallback(&request, &config);
        assert_eq!(action, FallbackAction::ReDispatchToScout);
        assert_eq!(reason, FallbackReason::EarlyFailure);
    }

    #[test]
    fn short_prompt_late_failure_centralizes() {
        let request = make_request(50, 30, 100); // 30% completion
        let config = default_config();
        let (action, reason) = decide_fallback(&request, &config);
        assert_eq!(action, FallbackAction::CentralizedFallback);
        assert_eq!(reason, FallbackReason::CompletionThreshold);
    }

    #[test]
    fn short_prompt_exact_threshold_centralizes() {
        let request = make_request(50, 25, 100); // exactly 25%
        let config = default_config();
        let (action, reason) = decide_fallback(&request, &config);
        assert_eq!(action, FallbackAction::CentralizedFallback);
        assert_eq!(reason, FallbackReason::CompletionThreshold);
    }

    #[test]
    fn continuation_prompt_builds_correctly() {
        let request = make_request(50, 3, 100);
        let prompt = build_continuation_prompt(&request);
        assert_eq!(prompt, "Hello world tok0 tok1 tok2");
    }

    #[test]
    fn continuation_prompt_no_tokens() {
        let request = make_request(50, 0, 100);
        let prompt = build_continuation_prompt(&request);
        assert_eq!(prompt, "Hello world");
    }

    #[test]
    fn replay_event_captures_state() {
        let request = make_request(50, 10, 100);
        let event = create_replay_event(
            &request,
            FallbackAction::ReDispatchToScout,
            FallbackReason::EarlyFailure,
        );
        assert_eq!(event.request_id, "test-req-1");
        assert_eq!(event.input_token_count, 50);
        assert_eq!(event.tokens_generated_so_far, 10);
        assert!((event.completion_ratio - 0.1).abs() < 0.001);
        assert_eq!(event.original_scout_peer_id, Some("peer-abc".to_string()));
    }

    #[test]
    fn boundary_at_threshold_1024() {
        // Exactly at threshold → should NOT be long context
        let request = make_request(1024, 0, 100);
        let config = default_config();
        let (action, _) = decide_fallback(&request, &config);
        assert_eq!(action, FallbackAction::ReDispatchToScout);

        // One over threshold → IS long context
        let request = make_request(1025, 0, 100);
        let (action, reason) = decide_fallback(&request, &config);
        assert_eq!(action, FallbackAction::CentralizedFallback);
        assert_eq!(reason, FallbackReason::LongContext);
    }
}
