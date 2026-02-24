use axum::http::{HeaderName, HeaderValue, StatusCode};
use super::*;

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

// ─── Speculative Decoding Functions ────────────────────────────────────────

/// Wait for a scout draft submission with timeout.
pub(crate) async fn wait_for_scout_draft(
    state: &SharedState,
    work_id: &str,
    timeout_ms: u64,
) -> Option<ScoutDraft> {
    let start = now_ms();
    let timeout_deadline = start + timeout_ms as u128;

    loop {
        if now_ms() >= timeout_deadline {
            tracing::debug!("scout draft timeout for work_id: {}", work_id);
            return None;
        }

        // Check if there's a draft available
        let draft = {
            let mut rx_guard = state.scout_draft_rx.lock().await;
            if let Some(rx) = rx_guard.as_mut() {
                match tokio::time::timeout(tokio::time::Duration::from_millis(100), rx.recv()).await
                {
                    Ok(Some(draft)) if draft.work_id == work_id => Some(draft),
                    _ => None,
                }
            } else {
                None
            }
        };

        if draft.is_some() {
            return draft;
        }

        // Small yield to prevent busy loop
        tokio::task::yield_now().await;
    }
}

/// Verify draft tokens against the verifier model.
/// Returns accepted tokens, text, and optionally a token to resample.
pub(crate) async fn verify_draft_tokens(
    engine: &mut shard_verifier::inference::ShardEngine,
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
            // Either greedy match OR the draft token is within 1.0 log-probability of the best
            let draft_logit = logits.get(draft_token as usize).copied().unwrap_or(-f32::INFINITY);
            let is_accepted = best_idx == draft_token as usize || (best_val - draft_logit) < 1.0;

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

    let rate_limit = state
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

    let request_id = format!("req-{}", now_ms());

    if stream_mode {
        let stream = async_stream::stream! {
            let mut engine_guard = state.engine.lock().await;
            if let Some(engine) = engine_guard.as_mut() {
                if let Ok(mut tokens) = engine.tokenize(&prompt, 4096) {
                    if !tokens.is_empty() && tokens[0] == 128000 {
                        tokens.remove(0);
                    }

                    // Speculative decoding: try to get scout draft
                    let mut accepted_text = String::new();
                    let prompt_tokens = tokens.clone();

                    if use_speculative {
                        if let Some(ref config) = speculative_config {
                            // Dispatch work to scouts
                            let work = WorkRequest {
                                request_id: request_id.clone(),
                                prompt_context: prompt.clone(),
                                min_tokens: config.draft_token_count as i32,
                                created_at_ms: Some(now_ms()),
                            };

                            {
                                let mut queue = state.scout_work.lock().await;
                                queue.push_back(work.clone());
                                // Keep queue size in check
                                while queue.len() > 1024 {
                                    queue.pop_front();
                                }
                            }

                            // Signal work availability (gossipsub)
                            if let Err(e) = state.work_tx.send(work).await {
                                tracing::warn!("failed to broadcast work: {}", e);
                            }

                            // Wait for scout draft with timeout
                            let draft_start = now_ms();
                            let draft = wait_for_scout_draft(&state, &request_id, config.scout_timeout_ms).await;
                            let draft_latency = (now_ms() - draft_start) as u64;
                            if let Some(mut draft) = draft {
                                // Record the measured latency
                                draft.latency_ms = draft_latency;
                                // Verify the draft against our model
                                let result = verify_draft_tokens(engine, &prompt_tokens, &draft.draft_tokens).await;
                                let accepted_count = result.accepted_tokens.len() as u64;
                                let draft_count = draft.draft_tokens.len() as u64;
                                let rejected_count = draft_count.saturating_sub(accepted_count);
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
                                    let chunk = serde_json::json!({
                                        "id": request_id,
                                        "object": "chat.completion.chunk",
                                        "created": now_ms() / 1000,
                                        "model": req.model.as_deref().unwrap_or("shard-hybrid"),
                                        "choices": [{"index": 0, "delta": {"content": result.accepted_text}, "finish_reason": serde_json::Value::Null}],
                                    });
                                    yield Ok::<_, std::convert::Infallible>(Event::default().data(chunk.to_string()));
                                    accepted_text.push_str(&result.accepted_text);
                                }

                                // Record success
                                let mut tracker = state.scout_timeout_tracker.lock().await;
                                tracker.record_success();
                            } else {
                                // Handle timeout
                                let in_cooldown = handle_scout_timeout(&state, config).await;
                                if in_cooldown {
                                    tracing::info!("scout in cooldown, using local generation");
                                }
                            }
                        }
                    }

                    if engine.eval(&tokens).is_ok() {
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

                                if let Ok(piece) = engine.token_to_piece(best_idx as i32) {
                                    let chunk = serde_json::json!({
                                        "id": request_id,
                                        "object": "chat.completion.chunk",
                                        "created": now_ms() / 1000,
                                        "model": req.model.as_deref().unwrap_or("shard-hybrid"),
                                        "choices": [{"index": 0, "delta": {"content": piece}, "finish_reason": serde_json::Value::Null}],
                                    });
                                    yield Ok::<_, std::convert::Infallible>(Event::default().data(chunk.to_string()));
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
                "model": req.model.as_deref().unwrap_or("shard-hybrid"),
                "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
            });
            yield Ok(Event::default().data(final_chunk.to_string()));
            yield Ok(Event::default().data("[DONE]"));
        };

        Sse::new(stream)
            .keep_alive(KeepAlive::default())
            .into_response()
    } else {
        // Run synchronously but hold the lock
        let mut full_text = String::new();
        {
            let mut engine_guard = state.engine.lock().await;
            if let Some(engine) = engine_guard.as_mut() {
                if let Ok(mut tokens) = engine.tokenize(&prompt, 4096) {
                    if !tokens.is_empty() && tokens[0] == 128000 {
                        tokens.remove(0);
                    }

                    // Speculative decoding: try to get scout draft
                    let prompt_tokens = tokens.clone();

                    if use_speculative {
                        if let Some(ref config) = speculative_config {
                            // Wait for scout draft with timeout
                            let draft_start = now_ms();
                            let draft =
                                wait_for_scout_draft(&state, &request_id, config.scout_timeout_ms)
                                    .await;
                            let draft_latency = (now_ms() - draft_start) as u64;
                            if let Some(mut draft) = draft {
                                // Record the measured latency
                                draft.latency_ms = draft_latency;
                                // Verify the draft against our model
                                let result = verify_draft_tokens(
                                    engine,
                                    &prompt_tokens,
                                    &draft.draft_tokens,
                                )
                                .await;
                                let accepted_count = result.accepted_tokens.len() as u64;
                                let draft_count = draft.draft_tokens.len() as u64;
                                let rejected_count = draft_count.saturating_sub(accepted_count);
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
                                    let status = penalties.apply_update(
                                        ScoutPenaltyUpdate {
                                            peer_id: draft.scout_id.clone(),
                                            accepted: result.first_rejection_idx.is_none(),
                                            probability_bound: 0.0,
                                            latency_ms: Some(draft.latency_ms),
                                            reason: result
                                                .first_rejection_idx
                                                .map(|idx| format!("Rejected at token {}", idx)),
                                        },
                                        p95,
                                    );

                                    if status.blackholed {
                                        let _ = state.ban_tx.try_send((draft.scout_id.clone(), status.last_reason.unwrap_or_else(|| "Repeated verification failure".to_string())));
                                    }
                                }

                                // Add accepted tokens to output
                                if !result.accepted_text.is_empty() {
                                    full_text.push_str(&result.accepted_text);
                                }

                                // Record success
                                let mut tracker = state.scout_timeout_tracker.lock().await;
                                tracker.record_success();
                            } else {
                                // Handle timeout
                                let in_cooldown = handle_scout_timeout(&state, config).await;
                                if in_cooldown {
                                    tracing::info!("scout in cooldown, using local generation");
                                }
                            }
                        }
                    }

                    if engine.eval(&tokens).is_ok() {
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

                                if let Ok(piece) = engine.token_to_piece(best_idx as i32) {
                                    full_text.push_str(&piece);
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

        Json(serde_json::json!({
            "id": request_id,
            "object": "chat.completion",
            "created": now_ms() / 1000,
            "model": req.model.as_deref().unwrap_or("shard-hybrid"),
            "choices": [{"index": 0, "message": {"role": "assistant", "content": full_text}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0}
        })).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::{auth_required, resolve_inference_mode, InferenceMode};

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
        assert_eq!(resolve_inference_mode(Some("standard")), InferenceMode::Standard);
        assert_eq!(resolve_inference_mode(None), InferenceMode::Standard);
        assert_eq!(resolve_inference_mode(Some("unknown")), InferenceMode::Standard);
    }

    #[test]
    fn auth_policy_matrix() {
        assert!(!auth_required(false, false));
        assert!(auth_required(true, false));
        assert!(auth_required(false, true));
        assert!(auth_required(true, true));
    }
}

