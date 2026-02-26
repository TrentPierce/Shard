use super::*;
use axum::http::{HeaderName, HeaderValue, StatusCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InferenceMode {
    Standard,
    Speculative,
}

pub(crate) fn resolve_inference_mode(raw: Option<&str>) -> InferenceMode {
    match raw.map(|value| value.trim().to_ascii_lowercase()) {
        Some(mode) if mode == "distributed" || mode == "speculative" => InferenceMode::Speculative,
        _ => InferenceMode::Standard,
    }
}

pub(crate) fn auth_required(require_api_key: bool, route_private: bool) -> bool {
    require_api_key || route_private
}

fn strip_control_tokens(raw: &str) -> String {
    // Truncate once model control markers begin to avoid leaking serialized
    // chat-template headers back to end users.
    if let Some(idx) = raw.find("<|") {
        raw[..idx].to_string()
    } else {
        raw.to_string()
    }
}

fn model_pair_acceptance_rates(
    draft_count: u64,
    accepted_count: u64,
    rejected_count: u64,
) -> (f64, f64) {
    if draft_count == 0 {
        return (1.0, 0.0);
    }
    (
        accepted_count as f64 / draft_count as f64,
        rejected_count as f64 / draft_count as f64,
    )
}

fn speculative_logit_tolerance() -> f32 {
    static TOL: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *TOL.get_or_init(|| {
        std::env::var("SHARD_SPECULATIVE_LOGIT_TOLERANCE")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| *v >= 0.0 && v.is_finite())
            .unwrap_or(4.0)
    })
}

const DEFAULT_SCOUT_WORK_QUEUE_MAX: usize = 1024;

fn scout_work_queue_max() -> usize {
    std::env::var("SHARD_SCOUT_WORK_QUEUE_MAX")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .map(|v| v.clamp(64, 4096))
        .unwrap_or(DEFAULT_SCOUT_WORK_QUEUE_MAX)
}

fn enqueue_scout_work(queue: &mut std::collections::VecDeque<WorkRequest>, work: WorkRequest) {
    let max_len = scout_work_queue_max();
    queue.push_back(work);
    while queue.len() > max_len {
        queue.pop_front();
    }
}

async fn dispatch_scout_work(state: &SharedState, work: WorkRequest) {
    {
        let mut queue = state.scout_work.lock().await;
        enqueue_scout_work(&mut queue, work.clone());
    }

    match state.work_tx.try_send(work) {
        Ok(_) => {}
        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
            tracing::warn!("work publish channel saturated; dropping scout broadcast");
        }
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
            tracing::warn!("work publish channel closed; unable to broadcast");
        }
    }
}

fn compute_effective_scout_timeout_ms(
    base_timeout_ms: u64,
    active_scouts: usize,
    queue_depth: usize,
) -> u64 {
    if active_scouts == 0 {
        return 0;
    }
    // WAN browser scouts can have high TTFT; allow longer waits by default.
    let bounded_base = base_timeout_ms.clamp(5_000, 120_000);
    if queue_depth > 768 {
        return bounded_base.min(20_000);
    }
    if active_scouts <= 1 {
        return bounded_base.min(90_000);
    }
    if active_scouts <= 3 {
        return bounded_base.min(75_000);
    }
    bounded_base.min(60_000)
}

async fn estimate_active_scouts(state: &SharedState) -> usize {
    let browser_sessions = state.browser_sessions.lock().await;
    let browser_count = browser_sessions.len();
    drop(browser_sessions);

    let recent_submitters = {
        let results = state.results.lock().await;
        let cutoff = now_ms().saturating_sub(3 * 60 * 1000);
        let mut unique = std::collections::HashSet::new();
        for entry in results.iter() {
            if entry.created_at_ms.unwrap_or(0) >= cutoff {
                unique.insert(entry.peer_id.clone());
            }
        }
        unique.len()
    };

    browser_count.max(recent_submitters)
}

async fn effective_speculative_timeout_ms(state: &SharedState, config: &SpeculativeConfig) -> u64 {
    let active_scouts = estimate_active_scouts(state).await;
    let queue_depth = {
        let queue = state.scout_work.lock().await;
        queue.len()
    };
    let timeout_ms =
        compute_effective_scout_timeout_ms(config.scout_timeout_ms, active_scouts, queue_depth);
    if timeout_ms == 0 {
        tracing::debug!("speculative dispatch skipped: no active scouts");
    }
    timeout_ms
}

async fn fetch_speculative_draft(
    state: &SharedState,
    request_id: &str,
    prompt: &str,
    config: &SpeculativeConfig,
) -> Option<ScoutDraft> {
    let work = WorkRequest {
        request_id: request_id.to_string(),
        prompt_context: prompt.to_string(),
        min_tokens: config.draft_token_count as i32,
        created_at_ms: Some(now_ms()),
    };

    let scout_timeout_ms = effective_speculative_timeout_ms(state, config).await;
    if scout_timeout_ms == 0 {
        tracing::debug!("skipping speculative dispatch with zero effective timeout");
        return None;
    }

    dispatch_scout_work(state, work).await;

    let draft_start = now_ms();
    let draft = wait_for_scout_draft(state, request_id, scout_timeout_ms).await;
    let draft_latency = (now_ms() - draft_start) as u64;
    if let Some(mut draft) = draft {
        draft.latency_ms = draft_latency;
        let mut tracker = state.scout_timeout_tracker.lock().await;
        tracker.record_success();
        Some(draft)
    } else {
        let in_cooldown = handle_scout_timeout(state, config).await;
        if in_cooldown {
            tracing::info!("scout in cooldown, using local generation");
        }
        None
    }
}

fn scout_draft_from_work_response(response: &WorkResponse) -> ScoutDraft {
    ScoutDraft {
        work_id: response.request_id.clone(),
        scout_id: response.peer_id.clone(),
        draft_tokens: response.draft_tokens.clone(),
        draft_text: response.draft_text.clone(),
        timestamp_ms: response.created_at_ms.unwrap_or_else(now_ms),
        latency_ms: response.latency_ms as u64,
    }
}

// â”€â”€â”€ Speculative Decoding Functions â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

async fn pop_mailbox_draft(state: &SharedState, work_id: &str) -> Option<ScoutDraft> {
    let mut mailbox = state.scout_draft_mailbox.lock().await;
    let draft = mailbox.get_mut(work_id).and_then(|queue| queue.pop_front());
    if mailbox
        .get(work_id)
        .map(|queue| queue.is_empty())
        .unwrap_or(false)
    {
        mailbox.remove(work_id);
    }
    draft
}

async fn get_or_create_draft_notifier(
    state: &SharedState,
    work_id: &str,
) -> Arc<tokio::sync::Notify> {
    let mut notifiers = state.scout_draft_notifiers.lock().await;
    notifiers
        .entry(work_id.to_string())
        .or_insert_with(|| Arc::new(tokio::sync::Notify::new()))
        .clone()
}

async fn clear_draft_notifier(state: &SharedState, work_id: &str) {
    let mut notifiers = state.scout_draft_notifiers.lock().await;
    notifiers.remove(work_id);
}

/// Wait for a scout draft submission with timeout.
pub(crate) async fn wait_for_scout_draft(
    state: &SharedState,
    work_id: &str,
    timeout_ms: u64,
) -> Option<ScoutDraft> {
    state.system_metrics.inc_speculative_wait_request();
    {
        let mut pending = state.speculative_pending.lock().await;
        pending.insert(work_id.to_string(), now_ms());
    }
    let start = now_ms();
    let timeout_deadline = start + timeout_ms as u128;

    loop {
        if now_ms() >= timeout_deadline {
            state.system_metrics.inc_speculative_wait_timeout();
            let age_ms = {
                let mut pending = state.speculative_pending.lock().await;
                pending
                    .remove(work_id)
                    .map(|issued| now_ms().saturating_sub(issued) as u64)
                    .unwrap_or(timeout_ms)
            };
            tracing::warn!(
                work_id = %work_id,
                timeout_ms,
                wait_age_ms = age_ms,
                "scout draft timeout"
            );
            clear_draft_notifier(state, work_id).await;
            return None;
        }

        if let Some(existing) = {
            let mut by_id = state.idempotent_results.lock().await;
            by_id.remove(work_id)
        } {
            state.system_metrics.inc_speculative_wait_hit();
            {
                let mut pending = state.speculative_pending.lock().await;
                pending.remove(work_id);
            }
            clear_draft_notifier(state, work_id).await;
            return Some(scout_draft_from_work_response(&existing));
        }

        if let Some(draft) = pop_mailbox_draft(state, work_id).await {
            state.system_metrics.inc_speculative_wait_hit();
            {
                let mut pending = state.speculative_pending.lock().await;
                pending.remove(work_id);
            }
            clear_draft_notifier(state, work_id).await;
            return Some(draft);
        }

        let notifier = get_or_create_draft_notifier(state, work_id).await;
        let remaining_ms = timeout_deadline.saturating_sub(now_ms()) as u64;
        let wait_ms = remaining_ms.clamp(50, 1000);
        let _ = tokio::time::timeout(
            tokio::time::Duration::from_millis(wait_ms),
            notifier.notified(),
        )
        .await;
    }
}

/// Verify draft tokens against the verifier model.
/// Returns accepted tokens, text, and optionally a token to resample.
pub(crate) async fn verify_draft_tokens(
    engine: &mut impl shard_verifier::inference::VerifierModel,
    prompt_tokens: &[i32],
    draft_tokens: &[i32],
) -> DraftVerificationResult {
    // 1. Evaluate the prompt context first to build KV cache
    if engine.eval(prompt_tokens).is_err() {
        return DraftVerificationResult {
            accepted_tokens: Vec::new(),
            accepted_text: String::new(),
            first_rejection_idx: None,
            resample_token: None,
        };
    }

    let mut accepted_tokens = Vec::new();
    let mut accepted_text = String::new();
    let mut first_rejection_idx = None;
    let vocab_size = 128256;

    // 2. Step through each draft token and check if the model would have predicted it
    let logit_tolerance = speculative_logit_tolerance();
    for (idx, &draft_token) in draft_tokens.iter().enumerate() {
        if let Ok(logits) = engine.get_logits(vocab_size) {
            // Find the argmax (greedy acceptance)
            let mut best_idx = 0;
            let mut best_val = -f32::INFINITY;
            for (i, &val) in logits.iter().enumerate() {
                if val > best_val {
                    best_val = val;
                    best_idx = i;
                }
            }

            // Probability bound check:
            // Either greedy match OR the draft token is within tolerance of best log-probability.
            let draft_logit = logits
                .get(draft_token as usize)
                .copied()
                .unwrap_or(-f32::INFINITY);
            let is_accepted =
                best_idx == draft_token as usize || (best_val - draft_logit) < logit_tolerance;

            if is_accepted {
                // Token accepted
                if let Ok(piece) = engine.token_to_piece(draft_token) {
                    accepted_text.push_str(&piece);
                }
                accepted_tokens.push(draft_token);

                // Advance engine with the accepted token to get logits for the next position
                if engine.eval(&[draft_token]).is_err() {
                    break;
                }
            } else {
                // First rejection
                first_rejection_idx = Some(idx);
                // The correct token to replace the rejected draft
                let resample_token = Some(best_idx as i32);
                return DraftVerificationResult {
                    accepted_tokens,
                    accepted_text,
                    first_rejection_idx,
                    resample_token,
                };
            }
        } else {
            break;
        }
    }

    DraftVerificationResult {
        accepted_tokens,
        accepted_text,
        first_rejection_idx,
        resample_token: None,
    }
}

/// Handle scout timeout - record in tracker and return whether in cooldown.
pub(crate) async fn handle_scout_timeout(state: &SharedState, config: &SpeculativeConfig) -> bool {
    let mut tracker = state.scout_timeout_tracker.lock().await;
    tracker.record_timeout(config);
    let in_cooldown = tracker.is_in_cooldown();

    if in_cooldown {
        tracing::warn!("scout in cooldown period, falling back to local generation");
    }

    in_cooldown
}

#[tracing::instrument(skip(state, req, headers))]
pub(crate) async fn chat_completions_handler(
    AxumState(state): AxumState<SharedState>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> impl IntoResponse {
    let route_private = headers
        .get("x-shard-route")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("private"))
        .unwrap_or(false);

    // 1. Authenticate API Key
    let api_key = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    if let Some(key) = api_key {
        let keys = state.api_keys.lock().await;
        if !keys.contains(key) {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "Invalid API Key" })),
            )
                .into_response();
        }
    } else if auth_required(state.require_api_key, route_private) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Authentication required by policy" })),
        )
            .into_response();
    }
    if route_private {
        state.system_metrics.inc_private_route();
    }

    // 2. Check Contribution Balance and Rate Limit
    let contribution_subject = api_key.unwrap_or("anonymous");
    let contribution_balance = if api_key.is_some() {
        let ledger = state.ledger.lock().await;
        ledger.balance_of(contribution_subject)
    } else {
        0
    };

    let rate_limit =
        state
            .rate_limiter
            .check(Some(contribution_subject), None, contribution_balance);
    if !rate_limit.is_allowed() {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            HeaderMap::from_iter([(
                HeaderName::from_static("retry-after"),
                HeaderValue::from_str(&rate_limit.retry_after().unwrap_or(60).to_string()).unwrap(),
            )]),
            Json(serde_json::json!({
                "error": "Rate limit exceeded. Contribute compute to increase your quota.",
                "contribution_balance": contribution_balance,
                "limit": rate_limit.limit(),
            })),
        )
            .into_response();
    }

    let stream_mode = req.stream.unwrap_or(false);
    let max_tokens = req.max_tokens.or(req.max_new_tokens).unwrap_or(256);

    let inference_mode = resolve_inference_mode(
        headers
            .get("x-shard-inference-mode")
            .and_then(|v| v.to_str().ok()),
    );
    let use_speculative = inference_mode == InferenceMode::Speculative;

    let speculative_config = if use_speculative {
        Some(SpeculativeConfig::default())
    } else {
        None
    };

    let mut prompt = String::new();
    prompt.push_str("<|begin_of_text|>");
    for msg in &req.messages {
        prompt.push_str(&format!(
            "<|start_header_id|>{}\n\n{}<|eot_id|>",
            msg.role, msg.content
        ));
    }
    prompt.push_str("<|start_header_id|>assistant<|end_header_id|>\n\n");

    let request_id = format!("req-{}", uuid::Uuid::new_v4());
    let request_started_ms = now_ms();
    let requested_draft_model = req
        .model
        .clone()
        .unwrap_or_else(|| "shard-hybrid".to_string());
    let rollout_decision = {
        let rollout = state.canary_rollout.lock().await;
        let canary_eligible = shard_verifier::inference::is_model_pair_compatible(
            requested_draft_model.as_str(),
            rollout.canary_model_id(),
        );
        rollout.decide(request_id.as_str(), canary_eligible)
    };
    let selected_verifier_model = rollout_decision.selected_model_id.clone();
    let model_pair_compatible = shard_verifier::inference::is_model_pair_compatible(
        requested_draft_model.as_str(),
        selected_verifier_model.as_str(),
    );
    let use_speculative = inference_mode == InferenceMode::Speculative && model_pair_compatible;

    if stream_mode {
        let stream = async_stream::stream! {
            let mut request_acceptance: Option<(f64, f64)> = None;
            let mut completion_tokens_generated: u64 = 0;
            let mut speculative_draft = if use_speculative {
                if let Some(ref config) = speculative_config {
                    fetch_speculative_draft(&state, &request_id, &prompt, config).await
                } else {
                    None
                }
            } else {
                None
            };
            let mut engine_guard = state.engine.lock().await;
            if let Some(engine) = engine_guard.as_mut() {
                if let Ok(mut tokens) = engine.tokenize(&prompt, 4096) {
                    if !tokens.is_empty() && tokens[0] == 128000 {
                        tokens.remove(0);
                    }

                    // Speculative decoding: try to get scout draft
                    let mut accepted_text = String::new();
                    let prompt_tokens = tokens.clone();
                    let mut prompt_already_evaluated = false;

                    if use_speculative {
                        if let Some(draft) = speculative_draft.take() {
                                // Verify the draft against our model
                                state.system_metrics.inc_speculative_verify_attempt();
                                let result = verify_draft_tokens(engine, &prompt_tokens, &draft.draft_tokens).await;
                                let accepted_count = result.accepted_tokens.len() as u64;
                                let draft_count = draft.draft_tokens.len() as u64;
                                let rejected_count = draft_count.saturating_sub(accepted_count);
                                if draft_count > 0 && accepted_count == 0 {
                                    state.system_metrics.inc_speculative_verify_zero_accept();
                                }
                                request_acceptance = Some(model_pair_acceptance_rates(
                                    draft_count,
                                    accepted_count,
                                    rejected_count,
                                ));
                                completion_tokens_generated = completion_tokens_generated
                                    .saturating_add(accepted_count);
                                prompt_already_evaluated = true;
                                state.system_metrics.inc_speculative_draft_tokens(draft_count);
                                state.system_metrics.inc_speculative_accepted_tokens(accepted_count);
                                state.system_metrics.inc_speculative_rejected_tokens(rejected_count);

                                if !result.accepted_tokens.is_empty() {
                                    let mut ledger = state.ledger.lock().await;
                                    let receipt = LedgerState::sign_poc_receipt(
                                        &state.signing_key,
                                        &draft.work_id,
                                        &draft.scout_id,
                                        result.accepted_tokens.len() as u32,
                                        now_ms()
                                    );
                                    let _ = ledger.apply_poc_receipt(receipt);
                                }

                                {
                                    let p95 = state.gossipsub_latency_hist.percentiles().p95_ms;
                                    let mut penalties = state.scout_penalties.lock().await;
                                    let status = penalties.apply_update(ScoutPenaltyUpdate {
                                        peer_id: draft.scout_id.clone(),
                                        accepted: result.first_rejection_idx.is_none(),
                                        probability_bound: 0.0,
                                        latency_ms: Some(draft.latency_ms),
                                        reason: result.first_rejection_idx.map(|idx| format!("Rejected at token {}", idx)),
                                    }, p95);

                                    if status.blackholed {
                                        let _ = state.ban_tx.try_send((draft.scout_id.clone(), status.last_reason.unwrap_or_else(|| "Repeated verification failure".to_string())));
                                    }
                                }

                                // Emit accepted tokens
                                if !result.accepted_text.is_empty() {
                                    let clean = strip_control_tokens(result.accepted_text.as_str());
                                    if !clean.is_empty() {
                                        let chunk = serde_json::json!({
                                            "id": request_id,
                                            "object": "chat.completion.chunk",
                                            "created": now_ms() / 1000,
                                            "model": selected_verifier_model.as_str(),
                                            "choices": [{"index": 0, "delta": {"content": clean}, "finish_reason": serde_json::Value::Null}],
                                        });
                                        yield Ok::<_, std::convert::Infallible>(Event::default().data(chunk.to_string()));
                                        accepted_text.push_str(clean.as_str());
                                    }
                                }

                        }
                    }

                    let prompt_ready = if prompt_already_evaluated {
                        true
                    } else {
                        engine.eval(&tokens).is_ok()
                    };
                    if prompt_ready {
                        let mut emitted = 0;
                        while emitted < max_tokens {
                            if let Ok(logits) = engine.get_logits(128256) {
                                let mut best_idx = 0;
                                let mut best_val = -f32::INFINITY;
                                for (i, &val) in logits.iter().enumerate() {
                                    if val > best_val {
                                        best_val = val;
                                        best_idx = i;
                                    }
                                }

                                if best_idx == 128001 || best_idx == 128009 {
                                    break;
                                }
                                completion_tokens_generated =
                                    completion_tokens_generated.saturating_add(1);

                                if let Ok(raw_piece) = engine.token_to_piece(best_idx as i32) {
                                    let piece = strip_control_tokens(raw_piece.as_str());
                                    if !piece.is_empty() {
                                        let chunk = serde_json::json!({
                                            "id": request_id,
                                            "object": "chat.completion.chunk",
                                            "created": now_ms() / 1000,
                                            "model": selected_verifier_model.as_str(),
                                            "choices": [{"index": 0, "delta": {"content": piece}, "finish_reason": serde_json::Value::Null}],
                                        });
                                        yield Ok::<_, std::convert::Infallible>(Event::default().data(chunk.to_string()));
                                    }
                                }

                                if engine.eval(&[best_idx as i32]).is_err() {
                                    break;
                                }
                                emitted += 1;
                            } else {
                                break;
                            }
                        }
                    }
                }
            } else {
                let err = serde_json::json!({"error": "No model engine loaded in this daemon"});
                yield Ok(Event::default().data(err.to_string()));
            }

            let final_chunk = serde_json::json!({
                "id": request_id,
                "object": "chat.completion.chunk",
                "created": now_ms() / 1000,
                "model": selected_verifier_model.as_str(),
                "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
            });
            yield Ok(Event::default().data(final_chunk.to_string()));
            yield Ok(Event::default().data("[DONE]"));

            state
                .system_metrics
                .inc_tokens_processed(completion_tokens_generated);

            let latency_ms = (now_ms().saturating_sub(request_started_ms)) as u64;
            let (acceptance_rate, reject_rate) = request_acceptance
                .map(|v| (Some(v.0), Some(v.1)))
                .unwrap_or((None, None));
            let mut rollout = state.canary_rollout.lock().await;
            rollout.record_request_outcome(
                &rollout_decision,
                latency_ms,
                acceptance_rate,
                reject_rate,
            );
        };

        Sse::new(stream)
            .keep_alive(KeepAlive::default())
            .into_response()
    } else {
        // Run synchronously but hold the lock
        let mut full_text = String::new();
        let mut request_acceptance: Option<(f64, f64)> = None;
        let mut prompt_token_count: u64 = 0;
        let mut completion_tokens_generated: u64 = 0;
        let mut speculative_draft = if use_speculative {
            if let Some(ref config) = speculative_config {
                fetch_speculative_draft(&state, &request_id, &prompt, config).await
            } else {
                None
            }
        } else {
            None
        };
        {
            let mut engine_guard = state.engine.lock().await;
            if let Some(engine) = engine_guard.as_mut() {
                if let Ok(mut tokens) = engine.tokenize(&prompt, 4096) {
                    if !tokens.is_empty() && tokens[0] == 128000 {
                        tokens.remove(0);
                    }
                    prompt_token_count = tokens.len() as u64;

                    // Speculative decoding: try to get scout draft
                    let prompt_tokens = tokens.clone();
                    let mut prompt_already_evaluated = false;

                    if use_speculative {
                        if let Some(draft) = speculative_draft.take() {
                                    // Verify the draft against our model
                                    state.system_metrics.inc_speculative_verify_attempt();
                                    let result = verify_draft_tokens(
                                        engine,
                                        &prompt_tokens,
                                        &draft.draft_tokens,
                                    )
                                    .await;
                                    let accepted_count = result.accepted_tokens.len() as u64;
                                    let draft_count = draft.draft_tokens.len() as u64;
                                    let rejected_count = draft_count.saturating_sub(accepted_count);
                                    if draft_count > 0 && accepted_count == 0 {
                                        state.system_metrics.inc_speculative_verify_zero_accept();
                                    }
                                    request_acceptance = Some(model_pair_acceptance_rates(
                                        draft_count,
                                        accepted_count,
                                        rejected_count,
                                    ));
                                    completion_tokens_generated =
                                        completion_tokens_generated.saturating_add(accepted_count);
                                    prompt_already_evaluated = true;
                                    state
                                        .system_metrics
                                        .inc_speculative_draft_tokens(draft_count);
                                    state
                                        .system_metrics
                                        .inc_speculative_accepted_tokens(accepted_count);
                                    state
                                        .system_metrics
                                        .inc_speculative_rejected_tokens(rejected_count);

                                    if !result.accepted_tokens.is_empty() {
                                        let mut ledger = state.ledger.lock().await;
                                        let receipt = LedgerState::sign_poc_receipt(
                                            &state.signing_key,
                                            &draft.work_id,
                                            &draft.scout_id,
                                            result.accepted_tokens.len() as u32,
                                            now_ms(),
                                        );
                                        let _ = ledger.apply_poc_receipt(receipt);
                                    }

                                    {
                                        let p95 = state.gossipsub_latency_hist.percentiles().p95_ms;
                                        let mut penalties = state.scout_penalties.lock().await;
                                        let status = penalties.apply_update(
                                            ScoutPenaltyUpdate {
                                                peer_id: draft.scout_id.clone(),
                                                accepted: result.first_rejection_idx.is_none(),
                                                probability_bound: 0.0,
                                                latency_ms: Some(draft.latency_ms),
                                                reason: result.first_rejection_idx.map(|idx| {
                                                    format!("Rejected at token {}", idx)
                                                }),
                                            },
                                            p95,
                                        );

                                        if status.blackholed {
                                            let _ = state.ban_tx.try_send((
                                                draft.scout_id.clone(),
                                                status.last_reason.unwrap_or_else(|| {
                                                    "Repeated verification failure".to_string()
                                                }),
                                            ));
                                        }
                                    }

                                    // Add accepted tokens to output
                                    if !result.accepted_text.is_empty() {
                                        let clean =
                                            strip_control_tokens(result.accepted_text.as_str());
                                        full_text.push_str(clean.as_str());
                                    }

                        }
                    }

                    let prompt_ready = if prompt_already_evaluated {
                        true
                    } else {
                        engine.eval(&tokens).is_ok()
                    };
                    if prompt_ready {
                        let mut emitted = 0;
                        while emitted < max_tokens {
                            if let Ok(logits) = engine.get_logits(128256) {
                                let mut best_idx = 0;
                                let mut best_val = -f32::INFINITY;
                                for (i, &val) in logits.iter().enumerate() {
                                    if val > best_val {
                                        best_val = val;
                                        best_idx = i;
                                    }
                                }

                                if best_idx == 128001 || best_idx == 128009 {
                                    break;
                                }
                                completion_tokens_generated =
                                    completion_tokens_generated.saturating_add(1);

                                if let Ok(piece) = engine.token_to_piece(best_idx as i32) {
                                    let clean = strip_control_tokens(piece.as_str());
                                    full_text.push_str(clean.as_str());
                                }

                                if engine.eval(&[best_idx as i32]).is_err() {
                                    break;
                                }
                                emitted += 1;
                            } else {
                                break;
                            }
                        }
                    }
                }
            } else {
                full_text = "No model engine loaded in this daemon".to_string();
            }
        }

        let latency_ms = (now_ms().saturating_sub(request_started_ms)) as u64;
        let (acceptance_rate, reject_rate) = request_acceptance
            .map(|v| (Some(v.0), Some(v.1)))
            .unwrap_or((None, None));
        {
            let mut rollout = state.canary_rollout.lock().await;
            rollout.record_request_outcome(
                &rollout_decision,
                latency_ms,
                acceptance_rate,
                reject_rate,
            );
        }

        state
            .system_metrics
            .inc_tokens_processed(completion_tokens_generated);

        let full_text = strip_control_tokens(full_text.as_str());
        let total_tokens = prompt_token_count.saturating_add(completion_tokens_generated);
        Json(serde_json::json!({
            "id": request_id,
            "object": "chat.completion",
            "created": now_ms() / 1000,
            "model": selected_verifier_model.as_str(),
            "canary": {
                "use_canary": rollout_decision.use_canary,
                "canary_eligible": rollout_decision.canary_eligible,
                "model_pair_compatible": model_pair_compatible,
            },
            "choices": [{"index": 0, "message": {"role": "assistant", "content": full_text}, "finish_reason": "stop"}],
            "usage": {
                "prompt_tokens": prompt_token_count,
                "completion_tokens": completion_tokens_generated,
                "total_tokens": total_tokens
            }
        })).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        auth_required, compute_effective_scout_timeout_ms, enqueue_scout_work,
        model_pair_acceptance_rates, resolve_inference_mode, strip_control_tokens, InferenceMode,
        WorkRequest,
    };
    use std::collections::VecDeque;

    #[test]
    fn distributed_mode_maps_to_speculative() {
        assert_eq!(
            resolve_inference_mode(Some("distributed")),
            InferenceMode::Speculative
        );
        assert_eq!(
            resolve_inference_mode(Some("speculative")),
            InferenceMode::Speculative
        );
    }

    #[test]
    fn standard_mode_disables_speculative() {
        assert_eq!(
            resolve_inference_mode(Some("standard")),
            InferenceMode::Standard
        );
        assert_eq!(resolve_inference_mode(None), InferenceMode::Standard);
        assert_eq!(
            resolve_inference_mode(Some("unknown")),
            InferenceMode::Standard
        );
    }

    #[test]
    fn auth_policy_matrix() {
        assert!(!auth_required(false, false));
        assert!(auth_required(true, false));
        assert!(auth_required(false, true));
        assert!(auth_required(true, true));
    }

    #[test]
    fn acceptance_rate_math_is_stable() {
        let (acceptance, reject) = model_pair_acceptance_rates(10, 7, 3);
        assert_eq!(acceptance, 0.7);
        assert_eq!(reject, 0.3);
        let (acceptance_empty, reject_empty) = model_pair_acceptance_rates(0, 0, 0);
        assert_eq!(acceptance_empty, 1.0);
        assert_eq!(reject_empty, 0.0);
    }

    #[test]
    fn strips_control_tokens_from_output() {
        let noisy = "Sure, how about you?<|eot_id|>user<|eot_id|><|start_header_id|>assistant";
        assert_eq!(strip_control_tokens(noisy), "Sure, how about you?");
    }

    #[test]
    fn strips_partial_control_token_tail() {
        let noisy = "Hello there<|end_header_id";
        assert_eq!(strip_control_tokens(noisy), "Hello there");
    }

    #[test]
    fn scout_work_queue_is_bounded() {
        let mut queue = VecDeque::new();
        for idx in 0..1100 {
            enqueue_scout_work(
                &mut queue,
                WorkRequest {
                    request_id: format!("req-{idx}"),
                    prompt_context: "p".to_string(),
                    min_tokens: 8,
                    created_at_ms: None,
                },
            );
        }
        assert_eq!(queue.len(), 1024);
        assert_eq!(queue.front().map(|w| w.request_id.as_str()), Some("req-76"));
        assert_eq!(
            queue.back().map(|w| w.request_id.as_str()),
            Some("req-1099")
        );
    }

    #[test]
    fn adaptive_timeout_short_circuits_without_active_scouts() {
        assert_eq!(compute_effective_scout_timeout_ms(30_000, 0, 0), 0);
        assert_eq!(compute_effective_scout_timeout_ms(30_000, 1, 0), 20_000);
        assert_eq!(compute_effective_scout_timeout_ms(30_000, 2, 0), 30_000);
        assert_eq!(compute_effective_scout_timeout_ms(30_000, 8, 900), 12_000);
    }
}
