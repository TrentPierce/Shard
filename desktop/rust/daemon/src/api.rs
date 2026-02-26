use super::*;

pub(crate) fn runtime_health_state(
    engine_loaded: bool,
    participation_enabled: bool,
    contribute_enabled: bool,
) -> (&'static str, &'static str, bool) {
    if !engine_loaded {
        ("degraded", "engine_unavailable", false)
    } else if !participation_enabled {
        ("degraded", "participation_disabled", false)
    } else if !contribute_enabled {
        ("degraded", "contribution_disabled", false)
    } else {
        ("ok", "ready", true)
    }
}

// ─── HTTP Control-Plane Handlers ────────────────────────────────────────────

pub(crate) async fn health_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let topo = state.topology.lock().await;
    let peers = state.peers.lock().await;
    let known = state.known_peers.lock().await;
    let browser_sessions = state.browser_sessions.lock().await;
    let verified_count = peers.values().filter(|p| p.verified).count();
    let capacity = state.capacity.load(Ordering::Relaxed);
    let load = state.current_load.load(Ordering::Relaxed);
    let latency_ms = state.avg_latency_ms.load(Ordering::Relaxed);
    let browser_scout_count = browser_sessions.len();
    let recent_scout_submitters = {
        let results = state.results.lock().await;
        let cutoff = now_ms().saturating_sub(5 * 60 * 1000);
        let mut unique = std::collections::HashSet::new();
        for entry in results.iter() {
            if entry.created_at_ms.unwrap_or(0) >= cutoff {
                unique.insert(entry.peer_id.clone());
            }
        }
        unique.len()
    };
    let scout_count = browser_scout_count.max(recent_scout_submitters);
    let model_compat =
        shard_verifier::inference::check_model_compatibility(state.model_id.as_str());
    let rollout_snapshot = {
        let rollout = state.canary_rollout.lock().await;
        rollout.snapshot()
    };
    let participation_enabled = state.participation_enabled.load(Ordering::Relaxed);
    let engine_guard = state.engine.lock().await;
    let engine_loaded = engine_guard.is_some();
    drop(engine_guard);
    let (status, readiness_reason, ready_for_inference) = runtime_health_state(
        engine_loaded,
        participation_enabled,
        topo.contribute_enabled,
    );

    Json(serde_json::json!({
        "status": status,
        "readiness_reason": readiness_reason,
        "ready_for_inference": ready_for_inference,
        "rust_sidecar": "connected",
        "rust_version": env!("CARGO_PKG_VERSION"),
        "rust_uptime_ms": now_ms() - state.daemon_start,
        "peer_id": topo.local_peer_id,
        "connected_peers": peers.len(),
        "verified_peers": verified_count,
        "active_scouts": scout_count,
        "active_browser_sessions": browser_scout_count,
        "recent_scout_submitters": recent_scout_submitters,
        "known_peers": known.len(),
        "uptime_ms": now_ms() - state.daemon_start,
        "listen_addrs": topo.listen_addrs,
        "public_api": topo.is_public,
        "public_api_addr": topo.public_api_addr,
        "relay_mode": topo.relay_server_enabled,
        "contribute": topo.contribute_enabled,
        "wallet": state.node_wallet.clone(),
        "model_id": state.model_id.clone(),
        "model_protocol_version": model_compat.protocol_version,
        "model_supports_speculative": model_compat.supports_speculative,
        "model_rollout": rollout_snapshot,
        "layer_start": state.layer_start,
        "layer_end": state.layer_end,
        "race_pool_size": state.race_pool_size,
        "race_timeout_ms": state.race_timeout_ms,
        "capacity": capacity,
        "load": load,
        "latency_ms": latency_ms,
        "engine_loaded": engine_loaded,
        "bitnet_model": std::env::var("BITNET_MODEL").unwrap_or_default(),
    }))
}

pub(crate) async fn connectivity_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let topo = state.topology.lock().await;

    // Determine NAT type based on topology state
    let nat_type = if topo.is_public {
        "none"
    } else if topo.relay_server_enabled {
        "full_cone"
    } else {
        "symmetric"
    };

    let relay_mode = topo.relay_server_enabled;
    let reachable_from_public = topo.is_public;

    // Generate recommended action based on connectivity
    let recommended_action = if reachable_from_public {
        "P2P direct connections available"
    } else if relay_mode {
        "Operating in relay mode - performance may be reduced"
    } else {
        "Consider enabling relay mode for better connectivity"
    };

    Json(serde_json::json!({
        "nat_type": nat_type,
        "relay_mode": relay_mode,
        "reachable_from_public": reachable_from_public,
        "recommended_action": recommended_action,
        "listen_addrs": topo.listen_addrs,
        "public_api_addr": topo.public_api_addr,
    }))
}

pub(crate) async fn node_status_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let topo = state.topology.lock().await;
    let logs = state.event_log.lock().await;
    let participation_enabled = state.participation_enabled.load(Ordering::Relaxed);
    let engine_guard = state.engine.lock().await;
    let engine_loaded = engine_guard.is_some();
    drop(engine_guard);
    let (_, readiness_reason, ready_for_inference) = runtime_health_state(
        engine_loaded,
        participation_enabled,
        topo.contribute_enabled,
    );
    Json(serde_json::json!({
        "ok": true,
        "node_role": state.node_role,
        "node_public_key": state.node_public_key,
        "participation_enabled": participation_enabled,
        "resource_policy": state.resource_policy,
        "current_load": state.current_load.load(Ordering::Relaxed),
        "capacity": state.capacity.load(Ordering::Relaxed),
        "latency_ms": state.avg_latency_ms.load(Ordering::Relaxed),
        "health_status": if ready_for_inference { "ready" } else { "degraded" },
        "readiness_reason": readiness_reason,
        "engine_loaded": engine_loaded,
        "peer_id": topo.local_peer_id,
        "recent_logs": logs.iter().cloned().collect::<Vec<String>>(),
    }))
}

pub(crate) async fn node_toggle_participation_handler(
    AxumState(state): AxumState<SharedState>,
    Json(toggle): Json<ParticipationToggle>,
) -> Json<serde_json::Value> {
    state
        .participation_enabled
        .store(toggle.enabled, Ordering::Relaxed);
    append_event_log(
        &state,
        format!("participation toggled to {}", toggle.enabled),
    )
    .await;
    Json(serde_json::json!({
        "ok": true,
        "participation_enabled": toggle.enabled,
    }))
}

pub(crate) async fn node_logs_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let logs = state.event_log.lock().await;
    Json(serde_json::json!({
        "ok": true,
        "logs": logs.iter().cloned().collect::<Vec<String>>(),
    }))
}

pub(crate) async fn node_ui_handler() -> Html<&'static str> {
    Html(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"/><meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>Shard Node UI</title>
<style>
body{font-family:"IBM Plex Sans","Segoe UI",sans-serif;background:#101a26;color:#e8f2ff;margin:0;padding:20px}
.card{background:#162536;border:1px solid #264a6e;border-radius:12px;padding:14px;margin-bottom:12px}
button{background:#2aa7df;color:#021019;border:0;padding:8px 10px;border-radius:8px;font-weight:700;cursor:pointer}
pre{white-space:pre-wrap;max-height:300px;overflow:auto}
</style></head><body>
<h1>Shard Node Control</h1>
<div class="card"><h3>Status</h3><pre id="status">loading...</pre></div>
<div class="card"><button onclick="toggle()">Toggle Participation</button></div>
<div class="card"><h3>Logs</h3><pre id="logs">loading...</pre></div>
<script>
async function refresh(){
  const s = await fetch('/node/status').then(r=>r.json());
  const l = await fetch('/node/logs').then(r=>r.json());
  document.getElementById('status').textContent = JSON.stringify(s,null,2);
  document.getElementById('logs').textContent = (l.logs||[]).join('\n');
}
async function toggle(){
  const s = await fetch('/node/status').then(r=>r.json());
  await fetch('/node/toggle-participation',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({enabled:!s.participation_enabled})});
  await refresh();
}
refresh(); setInterval(refresh, 4000);
</script></body></html>"#,
    )
}

pub(crate) async fn topology_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let topo = state.topology.lock().await;
    let (webrtc_addr, quic_addr, ws_addr, listen_addrs) = outward_topology_addrs(&topo, &state);
    let known = state.known_peers.lock().await;
    let capacity = state.capacity.load(Ordering::Relaxed);
    let load = state.current_load.load(Ordering::Relaxed);
    let latency_ms = state.avg_latency_ms.load(Ordering::Relaxed);
    Json(serde_json::json!({
        "status": "ok",
        "source": "rust-sidecar",
        "shard_peer_id": topo.local_peer_id,
        "shard_webrtc_multiaddr": webrtc_addr,
        "shard_quic_multiaddr": quic_addr,
        "shard_ws_multiaddr": ws_addr,
        "listen_addrs": listen_addrs,
        "known_peer_count": known.len(),
        "public_api": topo.is_public,
        "public_api_addr": state.public_host.clone().or(topo.public_api_addr.clone()),
        "relay_mode": topo.relay_server_enabled,
        "contribute": topo.contribute_enabled,
        "wallet": state.node_wallet.clone(),
        "model_id": state.model_id.clone(),
        "layer_start": state.layer_start,
        "layer_end": state.layer_end,
        "race_pool_size": state.race_pool_size,
        "race_timeout_ms": state.race_timeout_ms,
        "capacity": capacity,
        "load": load,
        "latency_ms": latency_ms,
        "ice_servers": state.ice_servers.lock().await.clone(),
    }))
}

pub(crate) async fn peers_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let peers = state.peers.lock().await;
    let list: Vec<&PeerInfo> = peers.values().collect();
    Json(serde_json::json!({ "peers": list, "count": list.len() }))
}

const BROWSER_SESSION_TTL_MS: u128 = 5 * 60 * 1000;
const DEFAULT_SCOUT_WORK_MAX_AGE_MS: u128 = 180_000;

fn scout_work_max_age_ms() -> u128 {
    static MAX_AGE_MS: std::sync::OnceLock<u128> = std::sync::OnceLock::new();
    *MAX_AGE_MS.get_or_init(|| {
        std::env::var("SHARD_SCOUT_WORK_MAX_AGE_MS")
            .ok()
            .and_then(|value| value.parse::<u128>().ok())
            .filter(|value| *value >= 1_000)
            .unwrap_or(DEFAULT_SCOUT_WORK_MAX_AGE_MS)
    })
}

pub(crate) fn parse_nonce_hex(raw: &str) -> Result<[u8; 12], String> {
    let bytes = hex::decode(raw).map_err(|e| format!("invalid nonce hex: {e}"))?;
    if bytes.len() != 12 {
        return Err("nonce must be 12 bytes".into());
    }
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&bytes);
    Ok(nonce)
}

pub(crate) fn prune_browser_sessions(
    sessions: &mut HashMap<String, BrowserLayerSession>,
    now: u128,
) {
    sessions.retain(|_, session| session.expires_at_ms > now);
}

pub(crate) async fn credits_balance_handler(
    AxumPath(wallet): AxumPath<String>,
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let ledger = state.ledger.lock().await;
    let balance = ledger.balance_of(wallet.as_str());
    Json(serde_json::json!({
        "ok": true,
        "wallet": wallet,
        "balance": balance,
    }))
}

pub(crate) async fn wallet_address_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "ok": true,
        "wallet": state.node_wallet.clone(),
    }))
}

pub(crate) async fn credits_tx_handler(
    AxumPath(tx_id): AxumPath<String>,
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let ledger = state.ledger.lock().await;
    match ledger.tx_by_id(tx_id.as_str()) {
        Some(tx) => Json(serde_json::json!({ "ok": true, "tx": tx })),
        None => Json(serde_json::json!({ "ok": false, "detail": "transaction not found" })),
    }
}

pub(crate) async fn ledger_head_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let ledger = state.ledger.lock().await;
    Json(serde_json::json!({
        "ok": true,
        "head": ledger.head(),
    }))
}

pub(crate) async fn ledger_stats_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let ledger = state.ledger.lock().await;
    Json(serde_json::json!({
        "ok": true,
        "stats": ledger.stats(),
    }))
}

pub(crate) async fn ledger_export_handler(
    AxumState(state): AxumState<SharedState>,
    Query(query): Query<LedgerExportQuery>,
) -> Json<serde_json::Value> {
    let from_height = query.from_height.unwrap_or(1);
    let limit = query.limit.unwrap_or(512).clamp(1, 4096);
    let ledger = state.ledger.lock().await;
    let export = ledger.export_range(from_height, limit);
    Json(serde_json::json!({
        "ok": true,
        "export": export,
    }))
}

pub(crate) async fn next_layer_handler(
    AxumState(state): AxumState<SharedState>,
    Query(query): Query<NextLayerQuery>,
) -> Json<serde_json::Value> {
    let model_id = query
        .model_id
        .as_deref()
        .unwrap_or(state.model_id.as_str())
        .to_string();
    let limit = query.limit.unwrap_or(3).clamp(1, 16);
    let model_pair_compatible = shard_verifier::inference::is_model_pair_compatible(
        model_id.as_str(),
        state.model_id.as_str(),
    );
    if !model_pair_compatible {
        return Json(serde_json::json!({
            "ok": true,
            "model_id": model_id,
            "current_layer": query.current_layer,
            "next_layer": query.current_layer.saturating_add(1),
            "peers": Vec::<String>::new(),
            "count": 0,
            "model_pair_compatible": false,
            "detail": "draft/verifier pair is not compatible for speculative scheduling",
        }));
    }

    let mut routes = state.layer_routes.lock().await;
    routes.prune_expired(now_ms());
    let peers = routes.find_next_layer_peers(&model_id, query.current_layer, limit);
    drop(routes);

    let snapshots = state.node_metric_reports.lock().await.clone();
    let reputation = state.node_reputation.lock().await.clone();
    let scheduler_inputs = peers
        .iter()
        .map(|peer_id| {
            let snapshot = snapshots.get(peer_id);
            let rep = reputation.get(peer_id);
            NodeSchedulerInput {
                node_id: peer_id.clone(),
                load: snapshot
                    .map(|s| (s.queue_depth as f64 / 64.0).min(1.0))
                    .unwrap_or(0.25),
                latency_ms: snapshot.map(|s| s.node_latency_ms as f64).unwrap_or(250.0),
                reliability_score: rep.map(NodeReputation::reliability_score).unwrap_or(0.8),
                hardware_capability_score: rep
                    .map(|r| r.hardware_capability_score)
                    .filter(|v| *v > 0.0)
                    .unwrap_or(0.7),
                identity_reputation_score: rep.map(|r| r.identity_score).unwrap_or(0.75),
            }
        })
        .collect::<Vec<_>>();
    let selected = weighted_select(scheduler_inputs.clone(), limit);
    record_scheduler_decision(
        &state,
        SchedulerDecisionLog {
            timestamp_ms: now_ms(),
            model_id: model_id.clone(),
            current_layer: query.current_layer,
            next_layer: query.current_layer.saturating_add(1),
            candidate_peers: peers.clone(),
            selected_peers: selected.clone(),
            inputs: scheduler_inputs
                .into_iter()
                .map(|input| SchedulerDecisionInput {
                    node_id: input.node_id,
                    load: input.load,
                    latency_ms: input.latency_ms,
                    reliability_score: input.reliability_score,
                    hardware_capability_score: input.hardware_capability_score,
                    identity_reputation_score: input.identity_reputation_score,
                })
                .collect(),
        },
    )
    .await;

    Json(serde_json::json!({
        "ok": true,
        "model_id": model_id,
        "current_layer": query.current_layer,
        "next_layer": query.current_layer.saturating_add(1),
        "peers": selected,
        "count": selected.len(),
        "model_pair_compatible": true,
    }))
}

pub(crate) async fn pipeline_forward_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<PipelineForwardRequest>,
) -> Json<serde_json::Value> {
    let model_id = req
        .model_id
        .as_deref()
        .unwrap_or(state.model_id.as_str())
        .to_string();
    let dispatch = PipelineDispatch {
        model_id,
        current_layer: req.current_layer,
        packet: req.packet,
    };
    match state.pipeline_tx.send(dispatch).await {
        Ok(_) => Json(serde_json::json!({
            "ok": true,
            "detail": "queued for pooled forward dispatch"
        })),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "detail": format!("channel error: {e}")
        })),
    }
}

pub(crate) async fn pipeline_pop_forward_result_handler(
    AxumState(state): AxumState<SharedState>,
    Query(query): Query<ForwardResultQuery>,
) -> Json<serde_json::Value> {
    let key = match (&query.request_id, &query.step_id) {
        (Some(r), Some(s)) => Some(RaceKey {
            request_id: r.clone(),
            step_id: s.clone(),
        }),
        _ => None,
    };

    let result = {
        let mut router = state.race_router.lock().await;
        router.pop_completed(key.as_ref())
    };

    match result {
        Some(res) => Json(serde_json::json!({ "ok": true, "result": res })),
        None => {
            Json(serde_json::json!({ "ok": false, "detail": "no completed race result available" }))
        }
    }
}

pub(crate) async fn browser_layer_register_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<BrowserLayerRegisterRequest>,
) -> Json<serde_json::Value> {
    if req.layer_end < req.layer_start {
        return Json(serde_json::json!({
            "ok": false,
            "detail": "layer_end must be >= layer_start",
        }));
    }
    if !req.profile.supports_webgpu {
        return Json(serde_json::json!({
            "ok": false,
            "detail": "webgpu support is required for browser layer hosting",
        }));
    }

    let model_id = req
        .model_id
        .as_deref()
        .unwrap_or(state.model_id.as_str())
        .to_string();
    let now = now_ms();
    let session_id = uuid::Uuid::new_v4().to_string();
    let obfuscation_key = rand::random::<[u8; 32]>().to_vec();
    let expires_at_ms = now + BROWSER_SESSION_TTL_MS;
    let session = BrowserLayerSession {
        session_id: session_id.clone(),
        model_id: model_id.clone(),
        layer_start: req.layer_start,
        layer_end: req.layer_end,
        profile: req.profile,
        obfuscation_key: obfuscation_key.clone(),
        last_seen_ms: now,
        expires_at_ms,
    };
    {
        let mut sessions = state.browser_sessions.lock().await;
        prune_browser_sessions(&mut sessions, now);
        sessions.insert(session_id.clone(), session);
    }

    Json(serde_json::json!(BrowserLayerRegisterResponse {
        ok: true,
        session_id,
        model_id,
        layer_start: req.layer_start,
        layer_end: req.layer_end,
        expires_at_ms,
        obfuscation_key_hex: hex::encode(obfuscation_key),
    }))
}

pub(crate) async fn browser_layer_work_handler(
    AxumState(state): AxumState<SharedState>,
    Query(query): Query<BrowserLayerWorkQuery>,
) -> Json<serde_json::Value> {
    let now = now_ms();
    {
        let mut sessions = state.browser_sessions.lock().await;
        prune_browser_sessions(&mut sessions, now);
        let Some(session) = sessions.get_mut(query.session_id.as_str()) else {
            return Json(serde_json::json!({"ok": false, "detail": "invalid session_id"}));
        };
        session.last_seen_ms = now;
        session.expires_at_ms = now + BROWSER_SESSION_TTL_MS;
    }

    let mut queue = state.browser_work.lock().await;
    if let Some(pos) = queue
        .iter()
        .position(|item| item.session_id == query.session_id)
    {
        if let Some(work) = queue.remove(pos) {
            return Json(serde_json::json!({
                "ok": true,
                "work": work,
            }));
        }
    }
    Json(serde_json::json!({"ok": true, "status": "empty"}))
}

pub(crate) async fn browser_layer_submit_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<BrowserLayerResultSubmit>,
) -> Json<serde_json::Value> {
    let key = {
        let mut sessions = state.browser_sessions.lock().await;
        prune_browser_sessions(&mut sessions, now_ms());
        let Some(session) = sessions.get_mut(req.session_id.as_str()) else {
            return Json(serde_json::json!({"ok": false, "detail": "invalid session_id"}));
        };
        session.last_seen_ms = now_ms();
        session.expires_at_ms = now_ms() + BROWSER_SESSION_TTL_MS;
        session.obfuscation_key.clone()
    };
    let topo_peer_id = state.topology.lock().await.local_peer_id.clone();

    let nonce = match parse_nonce_hex(req.nonce_hex.as_str()) {
        Ok(nonce) => nonce,
        Err(detail) => return Json(serde_json::json!({"ok": false, "detail": detail})),
    };
    let cipher = match hex::decode(req.obfuscated_tensor_hex.as_str()) {
        Ok(raw) => raw,
        Err(e) => {
            return Json(serde_json::json!({
                "ok": false,
                "detail": format!("invalid obfuscated_tensor_hex: {e}"),
            }))
        }
    };
    let plain = deobfuscate_bytes(&key, &nonce, &cipher);
    let wire = TensorWirePacket {
        tensor_name: req.tensor_name.clone(),
        dtype: req.dtype,
        shape: req.shape.clone(),
        data: plain,
    };
    let encoded = wire.encode();
    let response_packet = ForwardPassActivation {
        request_id: req.request_id.clone(),
        step_id: req.step_id.clone(),
        source_peer_id: topo_peer_id,
        target_peer_id: Some(req.source_peer_id.clone()),
        target_peer_pool: None,
        tensor_name: req.tensor_name,
        shape: req.shape.iter().map(|v| *v as usize).collect(),
        format: TensorDataFormat::Quantized,
        chunk: Some(TensorChunkRef {
            chunk_index: 0,
            total_chunks: 1,
            byte_size: encoded.len() as u64,
            checksum_blake3: None,
            data: format!("wire1:{}", hex::encode(encoded)),
        }),
        blob_ref: None,
        created_at_ms: Some(now_ms()),
    };

    match state.browser_result_tx.send(response_packet).await {
        Ok(_) => Json(serde_json::json!({
            "ok": true,
            "detail": "accepted browser activation result",
        })),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "detail": format!("failed to enqueue browser activation result: {e}"),
        })),
    }
}

pub(crate) async fn broadcast_work_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<WorkRequest>,
) -> Json<serde_json::Value> {
    process_work_request(&state, req).await
}

pub(crate) async fn signed_broadcast_work_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<SignedRequest<WorkRequest>>,
) -> Json<serde_json::Value> {
    let signer = req.envelope.signer_pubkey_hex.clone();
    if let Err(detail) = req.envelope.verify() {
        record_signature_alert(&state, false).await;
        state.system_metrics.inc_signature_verification_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": detail }));
    }
    if !accept_replay_nonce(
        &state.replay_nonces,
        &req.envelope.signer_pubkey_hex,
        req.envelope.nonce,
    )
    .await
    {
        state.system_metrics.inc_node_identity_auth_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": "stale or replayed nonce" }));
    }
    record_signature_alert(&state, true).await;
    process_work_request(&state, req.envelope.payload).await
}

#[tracing::instrument(skip(state, req), fields(id = %req.request_id))]
pub(crate) async fn process_work_request(
    state: &SharedState,
    req: WorkRequest,
) -> Json<serde_json::Value> {
    if let Err(detail) = validate_work_request(&req) {
        state.system_metrics.inc_task_failures();
        return Json(serde_json::json!({ "ok": false, "detail": detail }));
    }
    if let Err(detail) = should_accept_work(state) {
        state.system_metrics.inc_task_failures();
        return Json(serde_json::json!({ "ok": false, "detail": detail }));
    }

    // Phase B: Long-Context Routing Guard
    let token_count = {
        let mut engine_guard = state.engine.lock().await;
        if let Some(engine) = engine_guard.as_mut() {
            engine
                .tokenize(&req.prompt_context, 8192)
                .map(|t| t.len())
                .unwrap_or(0)
        } else {
            // If engine is not available, we can't count tokens precisely.
            // Fallback to character length approximation if needed, but here we'll just assume 0.
            // In a production app, we'd have a lightweight tokenizer.
            0
        }
    };

    if should_route_long_context(token_count, state.fallback_config.long_context_threshold) {
        tracing::info!(%token_count, threshold = state.fallback_config.long_context_threshold, "routing long prompt to centralized fallback");
        let active_state = ActiveRequestState {
            request_id: req.request_id.clone(),
            input_token_count: token_count,
            expected_output_tokens: req.min_tokens as usize,
            tokens_generated_so_far: 0,
            prompt_context: req.prompt_context.clone(),
            generated_tokens: Vec::new(),
            started_at_ms: now_ms(),
            scout_peer_id: None,
        };
        match execute_centralized_fallback(&state.fallback_config, &active_state).await {
            Ok(res) => {
                return Json(serde_json::json!({
                    "ok": true,
                    "result": res,
                    "fallback": true,
                    "reason": "LongContext"
                }))
            }
            Err(e) => {
                return Json(
                    serde_json::json!({ "ok": false, "detail": format!("fallback failed: {e}") }),
                )
            }
        }
    }

    {
        let mut queue = state.scout_work.lock().await;
        queue.push_back(req.clone());
        while queue.len() > 1024 {
            queue.pop_front();
        }
    }

    record_request_alert(state).await;

    match state.work_tx.send(req).await {
        Ok(_) => Json(serde_json::json!({ "ok": true, "detail": "queued for gossipsub publish" })),
        Err(e) => {
            state.system_metrics.inc_task_failures();
            Json(serde_json::json!({ "ok": false, "detail": format!("channel error: {e}") }))
        }
    }
}

pub(crate) async fn pop_result_handler(
    AxumState(state): AxumState<SharedState>,
    Query(query): Query<PopResultQuery>,
) -> Json<serde_json::Value> {
    if let Some(request_id) = query.request_id.clone() {
        let mut by_id = state.idempotent_results.lock().await;
        if let Some(result) = by_id.remove(&request_id) {
            state
                .system_metrics
                .inc_tokens_processed(result.draft_tokens.len() as u64);
            return Json(serde_json::json!({ "result": result }));
        }
    }

    let mut results = state.results.lock().await;
    if let Some(request_id) = query.request_id {
        if let Some(idx) = results.iter().position(|r| r.request_id == request_id) {
            if let Some(result) = results.remove(idx) {
                state
                    .system_metrics
                    .inc_tokens_processed(result.draft_tokens.len() as u64);
                return Json(serde_json::json!({ "result": result }));
            }
        }
        return Json(serde_json::json!({ "result": null }));
    }

    match results.pop_front() {
        Some(result) => {
            state
                .system_metrics
                .inc_tokens_processed(result.draft_tokens.len() as u64);
            Json(serde_json::json!({ "result": result }))
        }
        None => Json(serde_json::json!({ "result": null })),
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ScoutWorkQuery {
    scout_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ScoutClientEvent {
    scout_id: String,
    event: String,
    detail: Option<String>,
    status: Option<u16>,
}

fn require_scout_id(query: &ScoutWorkQuery) -> Result<&str, Json<serde_json::Value>> {
    match query
        .scout_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(id) => Ok(id),
        None => Err(Json(serde_json::json!({
            "ok": false,
            "detail": "scout_id is required",
        }))),
    }
}

pub(crate) async fn scout_client_event_handler(
    AxumState(state): AxumState<SharedState>,
    Json(event): Json<ScoutClientEvent>,
) -> Json<serde_json::Value> {
    let scout_id = event.scout_id.trim();
    if scout_id.is_empty() {
        return Json(serde_json::json!({
            "ok": false,
            "detail": "scout_id is required",
        }));
    }

    match event.event.as_str() {
        "submit_attempt" => state.system_metrics.inc_scout_client_submit_attempt(),
        "submit_success" => state.system_metrics.inc_scout_client_submit_success(),
        "submit_http_error" => state.system_metrics.inc_scout_client_submit_http_failure(),
        "submit_timeout" => state.system_metrics.inc_scout_client_submit_timeout(),
        "submit_pow_failure" => state.system_metrics.inc_scout_client_submit_pow_failure(),
        "submit_network_error" => state
            .system_metrics
            .inc_scout_client_submit_network_failure(),
        "generate_failure" => state.system_metrics.inc_scout_client_generate_failure(),
        "fallback_draft_used" => state.system_metrics.inc_scout_client_fallback_draft(),
        _ => {
            tracing::debug!(
                scout_id = %scout_id,
                event = %event.event,
                detail = ?event.detail,
                status = ?event.status,
                "ignored unknown scout client event"
            );
            return Json(serde_json::json!({
                "ok": false,
                "detail": "unknown event",
            }));
        }
    }

    Json(serde_json::json!({ "ok": true }))
}

pub(crate) async fn ensure_pow_verified(
    state: &SharedState,
    scout_id: &str,
) -> Result<(), Json<serde_json::Value>> {
    let mut manager = state.pow_manager.lock().await;
    manager.prune_expired();
    if manager.is_verified(scout_id) {
        Ok(())
    } else {
        state.system_metrics.inc_pow_challenges_failed();
        Err(Json(serde_json::json!({
            "ok": false,
            "detail": "pow verification required for scout ingress",
        })))
    }
}

pub(crate) async fn pop_work_handler(
    AxumState(state): AxumState<SharedState>,
    Query(query): Query<ScoutWorkQuery>,
) -> Json<serde_json::Value> {
    let scout_id = match require_scout_id(&query) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = ensure_pow_verified(&state, scout_id).await {
        return response;
    }
    state.system_metrics.inc_scout_work_poll();
    let mut queue = state.scout_work.lock().await;
    let now = now_ms();
    let max_age_ms = scout_work_max_age_ms();
    while let Some(work) = queue.pop_back() {
        let created_at_ms = work.created_at_ms.unwrap_or(now);
        let age_ms = now.saturating_sub(created_at_ms);
        if age_ms <= max_age_ms {
            state.system_metrics.inc_scout_work_assignment();
            return Json(serde_json::json!({ "work": work }));
        }
        tracing::debug!(
            request_id = %work.request_id,
            age_ms,
            max_age_ms,
            "dropping stale scout work item"
        );
    }
    state.system_metrics.inc_scout_work_empty_poll();
    Json(serde_json::json!({ "work": null }))
}

pub(crate) async fn submit_draft_handler(
    AxumState(state): AxumState<SharedState>,
    Json(submission): Json<DraftResultSubmission>,
) -> Json<serde_json::Value> {
    process_draft_submission(&state, submission).await
}

pub(crate) async fn signed_submit_draft_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<SignedRequest<DraftResultSubmission>>,
) -> Json<serde_json::Value> {
    let signer = req.envelope.signer_pubkey_hex.clone();
    if let Err(detail) = req.envelope.verify() {
        record_signature_alert(&state, false).await;
        state.system_metrics.inc_signature_verification_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": detail }));
    }
    if !accept_replay_nonce(
        &state.replay_nonces,
        &req.envelope.signer_pubkey_hex,
        req.envelope.nonce,
    )
    .await
    {
        state.system_metrics.inc_node_identity_auth_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": "stale or replayed nonce" }));
    }
    record_signature_alert(&state, true).await;
    process_draft_submission(&state, req.envelope.payload).await
}

pub(crate) async fn process_draft_submission(
    state: &SharedState,
    mut submission: DraftResultSubmission,
) -> Json<serde_json::Value> {
    submission.work_id = submission.work_id.trim().to_string();
    submission.scout_id = submission.scout_id.trim().to_string();
    state.system_metrics.inc_scout_draft_submission();
    if submission.work_id.trim().is_empty() || submission.scout_id.trim().is_empty() {
        state.system_metrics.inc_task_failures();
        state
            .system_metrics
            .inc_scout_draft_reject_missing_identity();
        if !submission.scout_id.trim().is_empty() {
            mark_node_failure(state, submission.scout_id.as_str()).await;
        }
        return Json(serde_json::json!({
            "ok": false,
            "detail": "work_id and scout_id are required",
        }));
    }
    if let Err(response) = ensure_pow_verified(state, submission.scout_id.as_str()).await {
        state.system_metrics.inc_task_failures();
        state.system_metrics.inc_scout_draft_reject_pow();
        mark_node_failure(state, submission.scout_id.as_str()).await;
        return response;
    }
    if let Some(spot_check) = submission.spot_check.as_ref() {
        if let Err(detail) = verify_spot_check_submission(spot_check) {
            state.system_metrics.inc_task_failures();
            state.system_metrics.inc_scout_draft_reject_spotcheck();
            mark_node_failure(state, submission.scout_id.as_str()).await;
            return Json(serde_json::json!({
                "ok": false,
                "detail": detail,
            }));
        }
    }

    submission.draft_text = sanitize_scout_draft_text(submission.draft_text.as_str());

    if submission.draft_tokens.is_empty() && !submission.draft_text.trim().is_empty() {
        let mut engine_guard = state.engine.lock().await;
        if let Some(engine) = engine_guard.as_mut() {
            if let Some(tokens) = tokenize_submission_draft(engine, &submission) {
                submission.draft_tokens = tokens;
            } else if let Ok(mut tokens) = engine.tokenize(submission.draft_text.as_str(), 256) {
                if !tokens.is_empty() && tokens[0] == 128000 {
                    tokens.remove(0);
                }
                submission.draft_tokens = tokens;
            }
        }
    }
    if submission.draft_tokens.is_empty() {
        state.system_metrics.inc_task_failures();
        state.system_metrics.inc_scout_draft_reject_empty_tokens();
        mark_node_failure(state, submission.scout_id.as_str()).await;
        return Json(serde_json::json!({
            "ok": false,
            "detail": "draft_tokens required or server tokenization must produce non-empty tokens",
        }));
    }

    let created_at_ms = submission
        .timestamp
        .map(|ts| (ts * 1000.0).max(0.0) as u128)
        .unwrap_or_else(now_ms);

    let response = WorkResponse {
        request_id: submission.work_id.clone(),
        peer_id: submission.scout_id.clone(),
        draft_tokens: submission.draft_tokens.clone(),
        draft_text: submission.draft_text.clone(),
        latency_ms: 0.0,
        created_at_ms: Some(created_at_ms),
    };

    {
        let mut by_id = state.idempotent_results.lock().await;
        if by_id.contains_key(response.request_id.as_str()) {
            state.system_metrics.inc_scout_draft_duplicate();
            mark_node_success(state, response.peer_id.as_str(), 0.0).await;
            return Json(serde_json::json!({
                "ok": true,
                "detail": "duplicate draft ignored (idempotent)",
            }));
        }
        by_id.insert(response.request_id.clone(), response.clone());
    }

    state
        .system_metrics
        .inc_tokens_offloaded_to_scouts(response.draft_tokens.len() as u64);

    let mut results = state.results.lock().await;
    results.push_back(response.clone());
    while results.len() > 2048 {
        results.pop_front();
    }

    mark_node_success(state, response.peer_id.as_str(), response.latency_ms as f64).await;

    // Forward to channel for synchronous waiting handlers (e.g. chat_completions_handler)
    let draft_for_channel = ScoutDraft {
        work_id: response.request_id.clone(),
        scout_id: response.peer_id.clone(),
        draft_tokens: response.draft_tokens.clone(),
        draft_text: submission.draft_text.clone(),
        timestamp_ms: response.created_at_ms.unwrap_or_else(now_ms),
        latency_ms: response.latency_ms as u64,
    };

    {
        let mut mailbox = state.scout_draft_mailbox.lock().await;
        let queue = mailbox
            .entry(draft_for_channel.work_id.clone())
            .or_insert_with(std::collections::VecDeque::new);
        queue.push_back(draft_for_channel.clone());
        while queue.len() > 8 {
            queue.pop_front();
        }
    }

    {
        let mut notifiers = state.scout_draft_notifiers.lock().await;
        let notify = notifiers
            .entry(draft_for_channel.work_id.clone())
            .or_insert_with(|| Arc::new(tokio::sync::Notify::new()))
            .clone();
        notify.notify_waiters();
    }

    match state.scout_draft_tx.try_send(draft_for_channel) {
        Ok(_) => {
            state.system_metrics.inc_scout_draft_channel_enqueued();
            Json(serde_json::json!({ "ok": true, "detail": "draft queued" }))
        }
        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
            state
                .system_metrics
                .inc_scout_draft_channel_enqueue_failure();
            tracing::warn!("scout draft channel full; using mailbox fallback");
            Json(serde_json::json!({
                "ok": true,
                "detail": "draft queued (mailbox fallback; channel saturated)",
            }))
        }
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
            state
                .system_metrics
                .inc_scout_draft_channel_enqueue_failure();
            tracing::warn!("scout draft channel closed; using mailbox fallback");
            Json(serde_json::json!({
                "ok": true,
                "detail": "draft queued (mailbox fallback; channel unavailable)",
            }))
        }
    }
}

fn sanitize_scout_draft_text(raw: &str) -> String {
    let marker_idx = raw.find("<|").unwrap_or(raw.len());
    raw[..marker_idx].replace('\0', "")
}

fn strip_optional_bos(tokens: &mut Vec<i32>) {
    if !tokens.is_empty() && tokens[0] == 128000 {
        tokens.remove(0);
    }
}

fn tokenize_submission_draft(
    engine: &mut impl shard_verifier::inference::VerifierModel,
    submission: &DraftResultSubmission,
) -> Option<Vec<i32>> {
    let prompt = submission.prompt_context.as_deref()?.trim();
    if prompt.is_empty() {
        return None;
    }

    let mut prompt_tokens = engine.tokenize(prompt, 4096).ok()?;
    strip_optional_bos(&mut prompt_tokens);

    let combined_text = format!("{prompt}{}", submission.draft_text);
    let mut combined_tokens = engine.tokenize(combined_text.as_str(), 4352).ok()?;
    strip_optional_bos(&mut combined_tokens);

    if combined_tokens.len() <= prompt_tokens.len() {
        return None;
    }
    if !combined_tokens.starts_with(prompt_tokens.as_slice()) {
        return None;
    }

    let mut continuation = combined_tokens[prompt_tokens.len()..].to_vec();
    if continuation.len() > 256 {
        continuation.truncate(256);
    }
    if continuation.is_empty() {
        return None;
    }
    Some(continuation)
}

fn should_route_long_context(token_count: usize, threshold: usize) -> bool {
    token_count > threshold
}

fn spot_check_config_from_env() -> shard_verifier::verification::spot_check::SpotCheckConfig {
    let defaults = shard_verifier::verification::spot_check::SpotCheckConfig::default();
    let sample_rate = std::env::var("SHARD_SPOTCHECK_SAMPLE_RATE")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .map(|v| v.clamp(0.0, 1.0))
        .unwrap_or(defaults.sample_rate);
    let tolerance = std::env::var("SHARD_SPOTCHECK_TOLERANCE")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| *v > 0.0)
        .unwrap_or(defaults.tolerance);
    let min_rows = std::env::var("SHARD_SPOTCHECK_MIN_ROWS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(defaults.min_rows);

    shard_verifier::verification::spot_check::SpotCheckConfig {
        sample_rate,
        tolerance,
        min_rows,
    }
}

fn verify_spot_check_submission(spot_check: &DraftSpotCheckProof) -> Result<(), String> {
    let config = spot_check_config_from_env();
    let result = shard_verifier::verification::spot_check::verify_matmul(
        &spot_check.input_a,
        &spot_check.weights_b,
        &spot_check.claimed_c,
        spot_check.m,
        spot_check.k,
        spot_check.n,
        &config,
        spot_check.seed.unwrap_or(42),
    );
    if result.passed {
        Ok(())
    } else {
        Err(format!(
            "spot-check verification failed: max_deviation={} tolerance={} failed_rows={:?}",
            result.max_deviation, result.tolerance, result.failed_rows
        ))
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct PowChallengeQuery {
    peer_id: String,
    hardware_concurrency: Option<usize>,
    is_mobile: Option<bool>,
}

pub(crate) async fn pow_challenge_handler(
    AxumState(state): AxumState<SharedState>,
    Query(query): Query<PowChallengeQuery>,
) -> Json<serde_json::Value> {
    state.system_metrics.inc_pow_challenges_issued();
    let mut manager = state.pow_manager.lock().await;
    let concurrency = query.hardware_concurrency.unwrap_or(4);
    let is_mobile = query.is_mobile.unwrap_or(false);
    let challenge = manager.issue_challenge(&query.peer_id, concurrency as u32, 60_000, is_mobile);
    Json(serde_json::json!({ "ok": true, "challenge": challenge }))
}

#[derive(Debug, Deserialize)]
pub(crate) struct PowVerifyRequest {
    peer_id: String,
    nonce: u64,
    hash_hex: String,
}

pub(crate) async fn pow_verify_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<PowVerifyRequest>,
) -> Json<serde_json::Value> {
    let mut manager = state.pow_manager.lock().await;
    use shard_common::common::pow_challenge::{PowSolution, PowVerifyResult};
    let result = manager.verify_solution(
        &req.peer_id,
        &PowSolution {
            nonce: req.nonce,
            hash_hex: req.hash_hex,
        },
    );
    let ok = matches!(result, PowVerifyResult::Accepted);
    if !ok {
        state.system_metrics.inc_pow_challenges_failed();
    }
    Json(serde_json::json!({ "ok": ok }))
}

pub(crate) async fn accept_replay_nonce(
    replay_nonces: &Arc<Mutex<HashMap<String, u64>>>,
    signer_pubkey_hex: &str,
    nonce: u64,
) -> bool {
    let mut guard = replay_nonces.lock().await;
    let previous = guard.get(signer_pubkey_hex).copied().unwrap_or(0);
    if nonce <= previous {
        return false;
    }
    guard.insert(signer_pubkey_hex.to_string(), nonce);
    true
}

pub(crate) async fn generate_local_fallback_tokens(
    state: &SharedState,
    prompt_context: &str,
    max_new_tokens: u32,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut engine_guard = state.engine.lock().await;
    let Some(engine) = engine_guard.as_mut() else {
        return out;
    };

    let Ok(mut tokens) = engine.tokenize(prompt_context, 4096) else {
        return out;
    };
    if !tokens.is_empty() && tokens[0] == 128000 {
        tokens.remove(0);
    }
    if engine.eval(&tokens).is_err() {
        return out;
    }

    let mut emitted = 0u32;
    while emitted < max_new_tokens {
        let Ok(logits) = engine.get_logits(128256) else {
            break;
        };

        let mut best_idx = 0usize;
        let mut best_val = -f32::INFINITY;
        for (idx, val) in logits.iter().enumerate() {
            if *val > best_val {
                best_val = *val;
                best_idx = idx;
            }
        }
        if best_idx == 128001 || best_idx == 128009 {
            break;
        }
        if let Ok(piece) = engine.token_to_piece(best_idx as i32) {
            out.push(piece);
        }
        if engine.eval(&[best_idx as i32]).is_err() {
            break;
        }
        emitted += 1;
    }
    out
}

pub(crate) async fn ws_generate_handler(
    ws: WebSocketUpgrade,
    AxumState(state): AxumState<SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_generate_stream(socket, state))
}

pub(crate) async fn ws_generate_stream(mut socket: WebSocket, state: SharedState) {
    let first = socket.recv().await;
    let Some(Ok(Message::Text(payload))) = first else {
        return;
    };

    let parsed: WsGenerateRequest = match serde_json::from_str(&payload) {
        Ok(req) => req,
        Err(err) => {
            let _ = socket
                .send(Message::Text(
                    serde_json::json!({"error": format!("invalid request: {err}")}).to_string(),
                ))
                .await;
            return;
        }
    };

    let request_id = parsed
        .request_id
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| format!("ws-{}", uuid::Uuid::new_v4()));
    let prompt_context = parsed
        .prompt
        .or(parsed.prompt_context)
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_default();
    let max_new_tokens = parsed.max_new_tokens.unwrap_or(128).clamp(1, 2048);

    let work = WorkRequest {
        request_id: request_id.clone(),
        prompt_context: prompt_context.clone(),
        min_tokens: 1,
        created_at_ms: Some(now_ms()),
    };

    if let Err(detail) = validate_work_request(&work) {
        state.system_metrics.inc_task_failures();
        let _ = socket
            .send(Message::Text(
                serde_json::json!({"error": detail, "event": "done"}).to_string(),
            ))
            .await;
        return;
    }
    if let Err(detail) = should_accept_work(&state) {
        state.system_metrics.inc_task_failures();
        let _ = socket
            .send(Message::Text(
                serde_json::json!({"error": detail, "event": "done"}).to_string(),
            ))
            .await;
        return;
    }

    {
        let mut queue = state.scout_work.lock().await;
        queue.push_back(work.clone());
        while queue.len() > 1024 {
            queue.pop_front();
        }
    }

    if state.work_tx.send(work).await.is_err() {
        state.system_metrics.inc_task_failures();
        state.system_metrics.inc_verification_fallback();
        let _ = socket
            .send(Message::Text(
                serde_json::json!({"error": "work queue unavailable", "event": "done"}).to_string(),
            ))
            .await;
        return;
    }

    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let mut emitted = 0u32;
    while emitted < max_new_tokens && tokio::time::Instant::now() < deadline {
        let maybe_result = {
            let mut results = state.results.lock().await;
            if let Some(idx) = results.iter().position(|r| r.request_id == request_id) {
                results.remove(idx)
            } else {
                None
            }
        };

        if let Some(result) = maybe_result {
            for token in result.draft_tokens {
                if emitted >= max_new_tokens {
                    break;
                }
                let payload = serde_json::json!({ "token": token }).to_string();
                if socket.send(Message::Text(payload)).await.is_err() {
                    return;
                }
                emitted += 1;
            }
            continue;
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    if emitted == 0 {
        state.system_metrics.inc_verification_fallback();
        let fallback =
            generate_local_fallback_tokens(&state, prompt_context.as_str(), max_new_tokens).await;
        for token in fallback {
            let payload = serde_json::json!({ "token": token }).to_string();
            if socket.send(Message::Text(payload)).await.is_err() {
                return;
            }
        }
    }

    let _ = socket
        .send(Message::Text(
            serde_json::json!({"event": "done"}).to_string(),
        ))
        .await;
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChatMessage {
    pub(crate) role: String,
    pub(crate) content: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChatRequest {
    pub(crate) model: Option<String>,
    pub(crate) messages: Vec<ChatMessage>,
    pub(crate) stream: Option<bool>,
    pub(crate) max_tokens: Option<u32>,
    pub(crate) max_new_tokens: Option<u32>,
}

pub(crate) async fn latency_profile_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let p = state.gossipsub_latency_hist.percentiles();
    Json(serde_json::json!({
        "source": "rust-sidecar",
        "gossipsub_propagation_ms": {
            "p50": p.p50_ms,
            "p90": p.p90_ms,
            "p99": p.p99_ms,
            "samples": p.samples,
        }
    }))
}

pub(crate) async fn metrics_handler(AxumState(state): AxumState<SharedState>) -> Response {
    let queue_depth = state.scout_work.lock().await.len();
    let active_node_count = state.node_metric_reports.lock().await.len();
    let node_latency_ms = state.avg_latency_ms.load(Ordering::Relaxed);
    let p = state.gossipsub_latency_hist.percentiles();
    let uptime_seconds = ((now_ms().saturating_sub(state.daemon_start)) / 1000) as u64;

    let body = state.system_metrics.render_prometheus(PrometheusSample {
        queue_depth,
        active_node_count,
        node_latency_ms,
        scheduler_decision_latency_ms: 0,
        e2e_latency_p50_ms: p.p50_ms,
        e2e_latency_p95_ms: p.p90_ms,
        e2e_latency_p99_ms: p.p99_ms,
        node_uptime_seconds: uptime_seconds,
    });

    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        body,
    )
        .into_response()
}

pub(crate) async fn alerts_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let alerts = {
        let manager = state.alert_manager.lock().await;
        manager.recent_alerts.iter().cloned().collect::<Vec<_>>()
    };
    Json(serde_json::json!({ "alerts": alerts }))
}

pub(crate) async fn private_mesh_register_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<PrivateMeshRegisterRequest>,
) -> Json<serde_json::Value> {
    let api_hash = hash_api_key(req.api_key.trim());
    let mut registry = state.private_mesh.lock().await;
    registry.register_node(&api_hash, req.node_pubkey_hex.as_str(), req.label.clone());
    Json(serde_json::json!({
        "ok": true,
        "group_count": registry.group_count(),
    }))
}

pub(crate) async fn private_mesh_deregister_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<PrivateMeshDeregisterRequest>,
) -> Json<serde_json::Value> {
    let api_hash = hash_api_key(req.api_key.trim());
    let mut registry = state.private_mesh.lock().await;
    let removed = registry.unregister_node(&api_hash, &req.node_pubkey_hex);
    Json(serde_json::json!({
        "ok": true,
        "removed": removed,
        "group_count": registry.group_count(),
    }))
}

pub(crate) async fn private_mesh_groups_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let groups = {
        let registry = state.private_mesh.lock().await;
        registry
            .list_groups()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>()
    };
    Json(serde_json::json!({
        "groups": groups,
        "group_count": groups.len(),
    }))
}

pub(crate) async fn private_mesh_route_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<PrivateMeshRouteRequest>,
) -> Json<serde_json::Value> {
    let api_hash = hash_api_key(req.api_key.trim());
    let connected: HashSet<String> = req.connected_peers.into_iter().collect();
    let decision = {
        let registry = state.private_mesh.lock().await;
        registry.route_private(&api_hash, &connected)
    };
    let payload = match decision {
        PrivateRouteDecision::DispatchToPrivateNodes(nodes) => serde_json::json!({
            "decision": "private_nodes",
            "nodes": nodes,
        }),
        PrivateRouteDecision::FallbackToCentralized => {
            serde_json::json!({ "decision": "fallback_to_centralized" })
        }
    };
    Json(serde_json::json!({
        "ok": true,
        "route": payload,
    }))
}

pub(crate) async fn admin_api_key_handler(
    AxumState(state): AxumState<SharedState>,
    headers: HeaderMap,
    Json(req): Json<AdminApiKeyRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let expected = match &state.admin_key {
        Some(key) => key,
        None => return Err(StatusCode::NOT_IMPLEMENTED),
    };
    let provided = headers
        .get("x-shard-admin")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    if provided.as_deref() != Some(expected) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let mut keys = state.api_keys.lock().await;
    let key = req
        .key
        .filter(|k| !k.trim().is_empty())
        .map(|k| k.trim().to_string())
        .unwrap_or_else(generate_api_key);
    keys.insert(key.clone());
    let total = keys.len();

    Ok(Json(serde_json::json!({
        "ok": true,
        "key": key,
        "total_keys": total,
    })))
}

pub(crate) async fn scout_penalty_update_handler(
    AxumState(state): AxumState<SharedState>,
    Json(update): Json<ScoutPenaltyUpdate>,
) -> impl IntoResponse {
    let mut penalties = state.scout_penalties.lock().await;
    let p95 = state.gossipsub_latency_hist.percentiles().p95_ms;
    let status = penalties.apply_update(update, p95);
    Json(status)
}

pub(crate) async fn scout_penalty_status_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let penalties = state.scout_penalties.lock().await;
    Json(serde_json::json!({
        "ok": true,
        "peers": penalties.all_statuses(),
    }))
}

pub(crate) fn node_is_healthy(last_report_ms: u128, now: u128, timeout_ms: u128) -> bool {
    now.saturating_sub(last_report_ms) <= timeout_ms
}

pub(crate) async fn upsert_node_snapshot(
    state: &SharedState,
    node_pubkey: String,
    role: String,
    queue_depth: u64,
    node_latency_ms: u64,
    uptime_seconds: u64,
    reported_at_ms: u128,
) {
    let now = now_ms();
    let healthy = node_is_healthy(reported_at_ms, now, state.heartbeat_timeout_ms);
    let mut reports = state.node_metric_reports.lock().await;
    reports.insert(
        node_pubkey.clone(),
        NodeMetricSnapshot {
            node_pubkey,
            role,
            queue_depth,
            node_latency_ms,
            uptime_seconds,
            last_report_ms: reported_at_ms,
            healthy,
        },
    );
}

pub(crate) async fn persist_reputation_map(
    reputation_path: PathBuf,
    reputation: HashMap<String, NodeReputation>,
) {
    let _ = tokio::task::spawn_blocking(move || {
        save_reputation(&reputation_path, &reputation);
    })
    .await;
}

pub(crate) async fn mark_node_success(state: &SharedState, node_id: &str, latency_ms: f64) {
    let (path, snapshot) = {
        let mut rep = state.node_reputation.lock().await;
        let entry = rep
            .entry(node_id.to_string())
            .or_insert_with(|| NodeReputation {
                identity_score: 0.8,
                hardware_capability_score: 0.7,
                ..NodeReputation::default()
            });
        entry.update_success(latency_ms, now_ms());
        (state.node_reputation_path.clone(), rep.clone())
    };
    persist_reputation_map(path, snapshot).await;

    record_latency_alert(state, latency_ms).await;
}

pub(crate) async fn mark_node_failure(state: &SharedState, node_id: &str) {
    let (path, snapshot) = {
        let mut rep = state.node_reputation.lock().await;
        let entry = rep
            .entry(node_id.to_string())
            .or_insert_with(|| NodeReputation {
                identity_score: 0.8,
                hardware_capability_score: 0.7,
                ..NodeReputation::default()
            });
        entry.update_failure(now_ms());
        (state.node_reputation_path.clone(), rep.clone())
    };
    persist_reputation_map(path, snapshot).await;
}

pub(crate) async fn signed_register_node_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<SignedRequest<NodeRegistration>>,
) -> Json<serde_json::Value> {
    let signer = req.envelope.signer_pubkey_hex.clone();
    if let Err(detail) = req.envelope.verify() {
        record_signature_alert(&state, false).await;
        state.system_metrics.inc_signature_verification_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": detail }));
    }

    if req.envelope.signer_pubkey_hex != req.envelope.payload.node_pubkey {
        state.system_metrics.inc_node_identity_auth_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({
            "ok": false,
            "detail": "signer pubkey does not match registration payload",
        }));
    }

    if !accept_replay_nonce(
        &state.replay_nonces,
        &req.envelope.signer_pubkey_hex,
        req.envelope.nonce,
    )
    .await
    {
        state.system_metrics.inc_node_identity_auth_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": "stale or replayed nonce" }));
    }
    record_signature_alert(&state, true).await;

    let ts = req.envelope.payload.timestamp_ms.unwrap_or_else(now_ms);
    upsert_node_snapshot(
        &state,
        req.envelope.payload.node_pubkey,
        req.envelope.payload.role,
        0,
        0,
        0,
        ts,
    )
    .await;
    mark_node_success(&state, &signer, 0.0).await;

    Json(serde_json::json!({ "ok": true, "detail": "registered" }))
}

pub(crate) async fn signed_heartbeat_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<SignedRequest<NodeHeartbeat>>,
) -> Json<serde_json::Value> {
    let signer = req.envelope.signer_pubkey_hex.clone();
    if let Err(detail) = req.envelope.verify() {
        record_signature_alert(&state, false).await;
        state.system_metrics.inc_signature_verification_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": detail }));
    }

    if req.envelope.signer_pubkey_hex != req.envelope.payload.node_pubkey {
        state.system_metrics.inc_node_identity_auth_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({
            "ok": false,
            "detail": "signer pubkey does not match heartbeat payload",
        }));
    }

    if !accept_replay_nonce(
        &state.replay_nonces,
        &req.envelope.signer_pubkey_hex,
        req.envelope.nonce,
    )
    .await
    {
        state.system_metrics.inc_node_identity_auth_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": "stale or replayed nonce" }));
    }

    record_signature_alert(&state, true).await;

    let heartbeat = req.envelope.payload;
    let ts = heartbeat.timestamp_ms.unwrap_or_else(now_ms);
    upsert_node_snapshot(
        &state,
        heartbeat.node_pubkey,
        heartbeat.role,
        heartbeat.queue_depth,
        heartbeat.node_latency_ms,
        heartbeat.uptime_seconds,
        ts,
    )
    .await;
    mark_node_success(&state, &signer, heartbeat.node_latency_ms as f64).await;

    Json(serde_json::json!({ "ok": true, "detail": "heartbeat accepted" }))
}

pub(crate) async fn signed_metrics_report_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<SignedRequest<NodeMetricReport>>,
) -> Json<serde_json::Value> {
    let signer = req.envelope.signer_pubkey_hex.clone();
    if let Err(detail) = req.envelope.verify() {
        record_signature_alert(&state, false).await;
        state.system_metrics.inc_signature_verification_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": detail }));
    }

    if req.envelope.signer_pubkey_hex != req.envelope.payload.node_pubkey {
        state.system_metrics.inc_node_identity_auth_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({
            "ok": false,
            "detail": "signer pubkey does not match metrics payload",
        }));
    }

    if !accept_replay_nonce(
        &state.replay_nonces,
        &req.envelope.signer_pubkey_hex,
        req.envelope.nonce,
    )
    .await
    {
        state.system_metrics.inc_node_identity_auth_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": "stale or replayed nonce" }));
    }
    record_signature_alert(&state, true).await;

    let report = req.envelope.payload;
    let ts = report.timestamp_ms.unwrap_or_else(now_ms);

    upsert_node_snapshot(
        &state,
        report.node_pubkey.clone(),
        report.role.clone(),
        report.queue_depth,
        report.node_latency_ms,
        report.uptime_seconds,
        ts,
    )
    .await;

    let persist = PersistedNodeMetricReport {
        node_pubkey: report.node_pubkey,
        role: report.role,
        queue_depth: report.queue_depth,
        node_latency_ms: report.node_latency_ms,
        uptime_seconds: report.uptime_seconds,
        timestamp_ms: ts,
    };
    if let Err(e) = state.metrics_persistence.persist_report(&persist).await {
        tracing::warn!(%e, "failed to persist metric report");
    }
    mark_node_success(&state, &signer, report.node_latency_ms as f64).await;

    Json(serde_json::json!({ "ok": true, "detail": "metrics report accepted" }))
}

pub(crate) async fn signed_deregister_node_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<SignedRequest<NodeRegistration>>,
) -> Json<serde_json::Value> {
    let signer = req.envelope.signer_pubkey_hex.clone();
    if let Err(detail) = req.envelope.verify() {
        record_signature_alert(&state, false).await;
        state.system_metrics.inc_signature_verification_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": detail }));
    }

    if req.envelope.signer_pubkey_hex != req.envelope.payload.node_pubkey {
        state.system_metrics.inc_node_identity_auth_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({
            "ok": false,
            "detail": "signer pubkey does not match deregistration payload",
        }));
    }

    if !accept_replay_nonce(
        &state.replay_nonces,
        &req.envelope.signer_pubkey_hex,
        req.envelope.nonce,
    )
    .await
    {
        state.system_metrics.inc_node_identity_auth_failures();
        mark_node_failure(&state, &signer).await;
        return Json(serde_json::json!({ "ok": false, "detail": "stale or replayed nonce" }));
    }
    record_signature_alert(&state, true).await;

    let mut reports = state.node_metric_reports.lock().await;
    reports.remove(req.envelope.payload.node_pubkey.as_str());
    Json(serde_json::json!({ "ok": true, "detail": "deregistered" }))
}

pub(crate) async fn metrics_summary_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let p = state.gossipsub_latency_hist.percentiles();
    let now = now_ms();
    let queue_depth = state.scout_work.lock().await.len();
    let mut reports = state.node_metric_reports.lock().await;
    let mut active_nodes = 0usize;
    let mut healthy_nodes = 0usize;
    let mut unhealthy_nodes = 0usize;

    for snapshot in reports.values_mut() {
        snapshot.healthy =
            node_is_healthy(snapshot.last_report_ms, now, state.heartbeat_timeout_ms);
        active_nodes += 1;
        if snapshot.healthy {
            healthy_nodes += 1;
        } else {
            unhealthy_nodes += 1;
        }
    }

    let counters = state.system_metrics.snapshot();
    let total_tokens = counters
        .tokens_processed_total
        .saturating_add(counters.tokens_offloaded_to_scouts_total);
    let offload_percentage = if total_tokens == 0 {
        0.0
    } else {
        (counters.tokens_offloaded_to_scouts_total as f64 / total_tokens as f64) * 100.0
    };
    let verification_rate = if counters.tokens_offloaded_to_scouts_total == 0 {
        100.0
    } else {
        ((counters.tokens_processed_total as f64
            / counters.tokens_offloaded_to_scouts_total as f64)
            * 100.0)
            .min(100.0)
    };
    let auth_failure_rate = if total_tokens == 0 {
        0.0
    } else {
        (counters.node_identity_auth_failures_total as f64 / total_tokens as f64) * 100.0
    };
    let speculative_acceptance_rate = if counters.speculative_draft_tokens_total == 0 {
        0.0
    } else {
        counters.speculative_accepted_tokens_total as f64
            / counters.speculative_draft_tokens_total as f64
    };
    let speculative_reject_rate = if counters.speculative_draft_tokens_total == 0 {
        0.0
    } else {
        counters.speculative_rejected_tokens_total as f64
            / counters.speculative_draft_tokens_total as f64
    };
    let speculative_speedup_ratio = if counters.speculative_accepted_tokens_total == 0 {
        1.0
    } else {
        1.0 + (counters.speculative_accepted_tokens_total as f64
            / (counters.speculative_accepted_tokens_total + 1) as f64)
    };
    let cost = estimate_cost(&CostEstimateInput {
        tokens_processed_total: counters.tokens_processed_total,
        offload_percent: offload_percentage,
        gpu_utilization_delta_percent: (offload_percentage * 0.6).min(90.0),
        cloud_gpu_usd_per_million_tokens: 4.0,
    });

    let mut payload = serde_json::Map::new();
    payload.insert("active_nodes".to_string(), serde_json::json!(active_nodes));
    payload.insert(
        "healthy_nodes".to_string(),
        serde_json::json!(healthy_nodes),
    );
    payload.insert(
        "unhealthy_nodes".to_string(),
        serde_json::json!(unhealthy_nodes),
    );
    payload.insert("queue_depth".to_string(), serde_json::json!(queue_depth));
    payload.insert(
        "node_identity_status".to_string(),
        serde_json::json!(if unhealthy_nodes == 0 {
            "ok"
        } else {
            "degraded"
        }),
    );
    payload.insert(
        "average_latency_ms".to_string(),
        serde_json::json!(state.avg_latency_ms.load(Ordering::Relaxed)),
    );
    payload.insert("p95_latency_ms".to_string(), serde_json::json!(p.p90_ms));
    payload.insert("p99_latency_ms".to_string(), serde_json::json!(p.p99_ms));
    payload.insert(
        "offload_percentage_estimate".to_string(),
        serde_json::json!(offload_percentage),
    );
    payload.insert(
        "verification_rate".to_string(),
        serde_json::json!(verification_rate),
    );
    payload.insert(
        "estimated_gpu_savings_percent".to_string(),
        serde_json::json!(cost.estimated_savings_percent),
    );
    payload.insert(
        "equivalent_cloud_gpu_cost_usd".to_string(),
        serde_json::json!(cost.equivalent_cloud_gpu_cost_usd),
    );
    payload.insert(
        "estimated_gpu_savings_usd".to_string(),
        serde_json::json!(cost.estimated_savings_usd),
    );
    payload.insert(
        "authentication_failure_rate".to_string(),
        serde_json::json!(auth_failure_rate),
    );
    payload.insert(
        "speculative_draft_tokens_total".to_string(),
        serde_json::json!(counters.speculative_draft_tokens_total),
    );
    payload.insert(
        "speculative_accepted_tokens_total".to_string(),
        serde_json::json!(counters.speculative_accepted_tokens_total),
    );
    payload.insert(
        "speculative_rejected_tokens_total".to_string(),
        serde_json::json!(counters.speculative_rejected_tokens_total),
    );
    payload.insert(
        "speculative_acceptance_rate".to_string(),
        serde_json::json!(speculative_acceptance_rate),
    );
    payload.insert(
        "speculative_reject_rate".to_string(),
        serde_json::json!(speculative_reject_rate),
    );
    payload.insert(
        "speculative_speedup_ratio".to_string(),
        serde_json::json!(speculative_speedup_ratio),
    );
    payload.insert(
        "scout_work_polls_total".to_string(),
        serde_json::json!(counters.scout_work_polls_total),
    );
    payload.insert(
        "scout_work_assignments_total".to_string(),
        serde_json::json!(counters.scout_work_assignments_total),
    );
    payload.insert(
        "scout_work_empty_polls_total".to_string(),
        serde_json::json!(counters.scout_work_empty_polls_total),
    );
    payload.insert(
        "scout_draft_submissions_total".to_string(),
        serde_json::json!(counters.scout_draft_submissions_total),
    );
    payload.insert(
        "scout_draft_reject_missing_identity_total".to_string(),
        serde_json::json!(counters.scout_draft_reject_missing_identity_total),
    );
    payload.insert(
        "scout_draft_reject_pow_total".to_string(),
        serde_json::json!(counters.scout_draft_reject_pow_total),
    );
    payload.insert(
        "scout_draft_reject_spotcheck_total".to_string(),
        serde_json::json!(counters.scout_draft_reject_spotcheck_total),
    );
    payload.insert(
        "scout_draft_reject_empty_tokens_total".to_string(),
        serde_json::json!(counters.scout_draft_reject_empty_tokens_total),
    );
    payload.insert(
        "scout_draft_duplicates_total".to_string(),
        serde_json::json!(counters.scout_draft_duplicates_total),
    );
    payload.insert(
        "scout_draft_channel_enqueued_total".to_string(),
        serde_json::json!(counters.scout_draft_channel_enqueued_total),
    );
    payload.insert(
        "scout_draft_channel_enqueue_failures_total".to_string(),
        serde_json::json!(counters.scout_draft_channel_enqueue_failures_total),
    );
    payload.insert(
        "speculative_wait_requests_total".to_string(),
        serde_json::json!(counters.speculative_wait_requests_total),
    );
    payload.insert(
        "speculative_wait_hits_total".to_string(),
        serde_json::json!(counters.speculative_wait_hits_total),
    );
    payload.insert(
        "speculative_wait_timeouts_total".to_string(),
        serde_json::json!(counters.speculative_wait_timeouts_total),
    );
    payload.insert(
        "speculative_wait_mismatched_work_id_total".to_string(),
        serde_json::json!(counters.speculative_wait_mismatched_work_id_total),
    );
    payload.insert(
        "speculative_verify_attempts_total".to_string(),
        serde_json::json!(counters.speculative_verify_attempts_total),
    );
    payload.insert(
        "speculative_verify_zero_accept_total".to_string(),
        serde_json::json!(counters.speculative_verify_zero_accept_total),
    );
    payload.insert(
        "scout_client_submit_attempts_total".to_string(),
        serde_json::json!(counters.scout_client_submit_attempts_total),
    );
    payload.insert(
        "scout_client_submit_success_total".to_string(),
        serde_json::json!(counters.scout_client_submit_success_total),
    );
    payload.insert(
        "scout_client_submit_http_failures_total".to_string(),
        serde_json::json!(counters.scout_client_submit_http_failures_total),
    );
    payload.insert(
        "scout_client_submit_timeouts_total".to_string(),
        serde_json::json!(counters.scout_client_submit_timeouts_total),
    );
    payload.insert(
        "scout_client_submit_pow_failures_total".to_string(),
        serde_json::json!(counters.scout_client_submit_pow_failures_total),
    );
    payload.insert(
        "scout_client_submit_network_failures_total".to_string(),
        serde_json::json!(counters.scout_client_submit_network_failures_total),
    );
    payload.insert(
        "scout_client_generate_failures_total".to_string(),
        serde_json::json!(counters.scout_client_generate_failures_total),
    );
    payload.insert(
        "scout_client_fallback_drafts_total".to_string(),
        serde_json::json!(counters.scout_client_fallback_drafts_total),
    );
    payload.insert(
        "transport_tcp_success_total".to_string(),
        serde_json::json!(counters.transport_tcp_success_total),
    );
    payload.insert(
        "transport_tcp_failure_total".to_string(),
        serde_json::json!(counters.transport_tcp_failure_total),
    );
    payload.insert(
        "transport_websocket_success_total".to_string(),
        serde_json::json!(counters.transport_websocket_success_total),
    );
    payload.insert(
        "transport_websocket_failure_total".to_string(),
        serde_json::json!(counters.transport_websocket_failure_total),
    );
    payload.insert(
        "transport_quic_success_total".to_string(),
        serde_json::json!(counters.transport_quic_success_total),
    );
    payload.insert(
        "transport_quic_failure_total".to_string(),
        serde_json::json!(counters.transport_quic_failure_total),
    );
    payload.insert(
        "transport_webrtc_success_total".to_string(),
        serde_json::json!(counters.transport_webrtc_success_total),
    );
    payload.insert(
        "transport_webrtc_failure_total".to_string(),
        serde_json::json!(counters.transport_webrtc_failure_total),
    );
    payload.insert(
        "transport_relay_success_total".to_string(),
        serde_json::json!(counters.transport_relay_success_total),
    );
    payload.insert(
        "transport_relay_failure_total".to_string(),
        serde_json::json!(counters.transport_relay_failure_total),
    );
    payload.insert(
        "tokens_processed_total".to_string(),
        serde_json::json!(counters.tokens_processed_total),
    );
    payload.insert(
        "tokens_offloaded_to_scouts_total".to_string(),
        serde_json::json!(counters.tokens_offloaded_to_scouts_total),
    );
    payload.insert(
        "verification_fallback_total".to_string(),
        serde_json::json!(counters.verification_fallback_total),
    );
    payload.insert(
        "task_failures_total".to_string(),
        serde_json::json!(counters.task_failures_total),
    );
    payload.insert(
        "signature_verification_failures_total".to_string(),
        serde_json::json!(counters.signature_verification_failures_total),
    );
    payload.insert(
        "node_identity_auth_failures_total".to_string(),
        serde_json::json!(counters.node_identity_auth_failures_total),
    );
    payload.insert(
        "nodes".to_string(),
        serde_json::to_value(
            reports
                .values()
                .cloned()
                .collect::<Vec<NodeMetricSnapshot>>(),
        )
        .unwrap_or_else(|_| serde_json::Value::Array(Vec::new())),
    );
    Json(serde_json::Value::Object(payload))
}

pub(crate) async fn dashboard_handler() -> Html<&'static str> {
    Html(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Shard Operations Dashboard</title>
  <style>
    :root { --bg:#09121a; --card:#132534; --fg:#d9e9f7; --muted:#8fb2cf; --accent:#44d2ff; --ok:#4ae596; --warn:#ffbe55; }
    body { margin:0; font-family: "IBM Plex Sans", "Segoe UI", sans-serif; background: radial-gradient(circle at top, #163149, var(--bg)); color: var(--fg); }
    main { max-width: 1080px; margin: 0 auto; padding: 24px; }
    h1 { margin: 0 0 18px 0; font-size: 28px; letter-spacing: .02em; }
    .grid { display:grid; grid-template-columns: repeat(auto-fit, minmax(220px,1fr)); gap: 14px; }
    .card { background: linear-gradient(180deg, #173147, var(--card)); border:1px solid #295170; border-radius: 12px; padding: 14px; }
    .label { color: var(--muted); font-size: 12px; text-transform: uppercase; letter-spacing: .08em; }
    .value { font-size: 26px; margin-top: 4px; }
    table { width:100%; border-collapse: collapse; margin-top: 16px; }
    th, td { text-align:left; padding: 8px; border-bottom: 1px solid #26465f; }
    .ok { color: var(--ok); } .warn { color: var(--warn); }
  </style>
</head>
<body>
<main>
  <h1>Shard Operations Dashboard</h1>
  <div class="grid" id="cards"></div>
  <table>
    <thead><tr><th>Node</th><th>Role</th><th>Queue</th><th>Latency</th><th>Uptime</th><th>Status</th></tr></thead>
    <tbody id="nodes"></tbody>
  </table>
</main>
<script>
async function refresh() {
  const res = await fetch('/metrics/summary', { cache: 'no-store' });
  const data = await res.json();
  const cards = [
    ['Active nodes', data.active_nodes],
    ['Healthy nodes', data.healthy_nodes],
    ['Average latency', data.average_latency_ms + ' ms'],
    ['P95 latency', data.p95_latency_ms + ' ms'],
    ['Queue depth', data.queue_depth],
    ['Identity status', data.node_identity_status],
  ];
  document.getElementById('cards').innerHTML = cards.map(([k,v]) => `<div class="card"><div class="label">${k}</div><div class="value">${v}</div></div>`).join('');
  document.getElementById('nodes').innerHTML = (data.nodes || []).map(n => `<tr><td>${n.node_pubkey.slice(0,16)}...</td><td>${n.role}</td><td>${n.queue_depth}</td><td>${n.node_latency_ms} ms</td><td>${n.uptime_seconds}s</td><td class="${n.healthy ? 'ok' : 'warn'}">${n.healthy ? 'healthy' : 'unhealthy'}</td></tr>`).join('');
}
refresh(); setInterval(refresh, 5000);
</script>
</body></html>"#,
    )
}

// ─── Bootstrap Discovery Handlers ─────────────────────────────────────────────

pub(crate) async fn bootstrap_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let peers = state.peers.lock().await;
    let topo = state.topology.lock().await;
    let now = now_ms();

    // Get stable peers that could be bootstraps
    let stable_peers: Vec<serde_json::Value> = peers
        .iter()
        .filter(|(_, p)| {
            let uptime_ms = now.saturating_sub(p.first_seen_at);
            let uptime_hours = uptime_ms / (1000 * 60 * 60);
            let total = p.successful_handshakes + p.handshake_failures;
            let failure_rate = if total > 0 {
                p.handshake_failures as f32 / total as f32
            } else {
                1.0
            };
            // Stable if: >1 hour uptime, >3 successful handshakes, <10% failure rate
            uptime_hours >= 1 && p.successful_handshakes >= 3 && failure_rate < 0.1
        })
        .map(|(id, p)| {
            let total = p.successful_handshakes + p.handshake_failures;
            let failure_rate = if total > 0 {
                p.handshake_failures as f32 / total as f32
            } else {
                0.0
            };
            let stability_score = (((1.0 - failure_rate) * 100.0) as u32).min(100);
            serde_json::json!({
                "peer_id": id,
                "multiaddr": p.addrs.first().cloned().unwrap_or_default(),
                "uptime_hours": (now.saturating_sub(p.first_seen_at)) / (1000 * 60 * 60),
                "stability_score": stability_score,
            })
        })
        .collect();

    // Calculate if THIS node is stable enough to be a bootstrap
    let my_uptime_hours = (now - state.daemon_start) / (1000 * 60 * 60);
    let is_bootstrap = my_uptime_hours >= 1;

    let registered = state.bootstrap_registry.lock().await;
    let persisted_bootstraps: Vec<serde_json::Value> = registered
        .values()
        .map(|entry| {
            serde_json::json!({
                "peer_id": entry.peer_id,
                "multiaddr": entry.multiaddr,
                "uptime_hours": entry.uptime_hours,
                "stability_score": entry.stability_score,
                "version": entry.version,
                "updated_at_ms": entry.updated_at_ms,
            })
        })
        .collect();

    Json(serde_json::json!({
        "local_peer_id": topo.local_peer_id,
        "is_bootstrap": is_bootstrap,
        "uptime_hours": my_uptime_hours,
        "known_bootstraps": stable_peers,
        "registered_bootstraps": persisted_bootstraps,
    }))
}

fn push_scheduler_decision_log(
    decisions: &mut VecDeque<SchedulerDecisionLog>,
    decision: SchedulerDecisionLog,
) {
    decisions.push_back(decision);
    while decisions.len() > 500 {
        decisions.pop_front();
    }
}

pub(crate) async fn record_scheduler_decision(state: &SharedState, decision: SchedulerDecisionLog) {
    let mut decisions = state.scheduler_decisions.lock().await;
    push_scheduler_decision_log(&mut decisions, decision);
}

pub(crate) async fn scheduler_decisions_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let decisions = state.scheduler_decisions.lock().await;
    Json(serde_json::json!({
        "ok": true,
        "count": decisions.len(),
        "decisions": decisions.iter().cloned().collect::<Vec<SchedulerDecisionLog>>(),
    }))
}

pub(crate) async fn model_rollout_status_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let rollout = state.canary_rollout.lock().await;
    Json(serde_json::json!({
        "ok": true,
        "rollout": rollout.snapshot(),
    }))
}

pub(crate) async fn model_rollout_reset_handler(
    AxumState(state): AxumState<SharedState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    if let Some(admin_key) = state.admin_key.as_deref() {
        let provided = headers
            .get("x-shard-admin")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .unwrap_or_default();
        if provided != admin_key {
            return Json(serde_json::json!({
                "ok": false,
                "detail": "admin key required for rollout reset",
            }));
        }
    }

    let mut rollout = state.canary_rollout.lock().await;
    rollout.reset_rollback();
    Json(serde_json::json!({
        "ok": true,
        "rollout": rollout.snapshot(),
    }))
}

#[derive(Deserialize)]
pub(crate) struct RegisterBootstrapRequest {
    pub peer_id: String,
    pub multiaddr: String,
    pub stability_score: u32,
    pub uptime_hours: u64,
    pub version: String,
}

pub(crate) async fn register_bootstrap_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<RegisterBootstrapRequest>,
) -> Json<serde_json::Value> {
    if req.peer_id.trim().is_empty() || req.multiaddr.trim().is_empty() {
        return Json(serde_json::json!({
            "ok": false,
            "detail": "peer_id and multiaddr are required",
        }));
    }
    if req.stability_score > 100 {
        return Json(serde_json::json!({
            "ok": false,
            "detail": "stability_score must be <= 100",
        }));
    }

    tracing::info!(peer_id = %req.peer_id, score = req.stability_score, "Registering bootstrap peer");
    let entry = BootstrapRegistryEntry {
        peer_id: req.peer_id.clone(),
        multiaddr: req.multiaddr.clone(),
        stability_score: req.stability_score,
        uptime_hours: req.uptime_hours,
        version: req.version.clone(),
        updated_at_ms: now_ms(),
    };
    let mut registry = state.bootstrap_registry.lock().await;
    registry.insert(req.peer_id.clone(), entry);
    save_bootstrap_registry(state.bootstrap_registry_path.as_path(), &registry).await;

    Json(serde_json::json!({
        "ok": true,
        "message": "Bootstrap registration recorded",
        "total_registered": registry.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        push_scheduler_decision_log, require_scout_id, runtime_health_state,
        sanitize_scout_draft_text, should_route_long_context, verify_spot_check_submission,
        DraftSpotCheckProof, ScoutWorkQuery,
    };
    use crate::SchedulerDecisionLog;
    use std::collections::VecDeque;

    #[test]
    fn scout_work_requires_identity() {
        let missing = ScoutWorkQuery { scout_id: None };
        assert!(require_scout_id(&missing).is_err());

        let empty = ScoutWorkQuery {
            scout_id: Some("  ".to_string()),
        };
        assert!(require_scout_id(&empty).is_err());

        let valid = ScoutWorkQuery {
            scout_id: Some("scout_123".to_string()),
        };
        assert_eq!(require_scout_id(&valid).ok(), Some("scout_123"));
    }

    #[test]
    fn spot_check_passes_for_correct_matmul() {
        let proof = DraftSpotCheckProof {
            input_a: vec![1.0, 2.0, 3.0, 4.0],   // 2x2
            weights_b: vec![1.0, 0.0, 0.0, 1.0], // 2x2 identity
            claimed_c: vec![1.0, 2.0, 3.0, 4.0], // A * I
            m: 2,
            k: 2,
            n: 2,
            seed: Some(7),
        };
        assert!(verify_spot_check_submission(&proof).is_ok());
    }

    #[test]
    fn spot_check_rejects_tampered_matmul() {
        let proof = DraftSpotCheckProof {
            input_a: vec![1.0, 2.0, 3.0, 4.0],       // 2x2
            weights_b: vec![1.0, 0.0, 0.0, 1.0],     // identity
            claimed_c: vec![10.0, 20.0, 30.0, 40.0], // tampered all elements
            m: 2,
            k: 2,
            n: 2,
            seed: Some(7),
        };
        assert!(verify_spot_check_submission(&proof).is_err());
    }

    #[test]
    fn scheduler_decision_log_is_bounded() {
        let mut decisions = VecDeque::new();
        for index in 0..520 {
            push_scheduler_decision_log(
                &mut decisions,
                SchedulerDecisionLog {
                    timestamp_ms: index,
                    model_id: "m".to_string(),
                    current_layer: 1,
                    next_layer: 2,
                    candidate_peers: vec!["a".to_string()],
                    selected_peers: vec!["a".to_string()],
                    inputs: vec![],
                },
            );
        }
        assert_eq!(decisions.len(), 500);
        assert_eq!(decisions.front().map(|d| d.timestamp_ms), Some(20));
        assert_eq!(decisions.back().map(|d| d.timestamp_ms), Some(519));
    }

    #[test]
    fn long_context_routing_threshold() {
        assert!(!should_route_long_context(1024, 2048));
        assert!(!should_route_long_context(2048, 2048));
        assert!(should_route_long_context(2049, 2048));
    }

    #[test]
    fn draft_verifier_pair_compatibility_guard() {
        assert!(shard_verifier::inference::is_model_pair_compatible(
            "shard-hybrid",
            "default-model"
        ));
        assert!(!shard_verifier::inference::is_model_pair_compatible(
            "legacy-draft-v0",
            "verifier-v2"
        ));
    }

    #[test]
    fn scout_draft_text_sanitization_stops_on_control_markers() {
        let raw = "Hello there<|eot_id|>user<|start_header_id|>assistant";
        assert_eq!(sanitize_scout_draft_text(raw), "Hello there");
    }

    #[test]
    fn runtime_health_state_reflects_model_readiness() {
        assert_eq!(
            runtime_health_state(false, true, true),
            ("degraded", "engine_unavailable", false)
        );
        assert_eq!(
            runtime_health_state(true, false, true),
            ("degraded", "participation_disabled", false)
        );
        assert_eq!(
            runtime_health_state(true, true, false),
            ("degraded", "contribution_disabled", false)
        );
        assert_eq!(
            runtime_health_state(true, true, true),
            ("ok", "ready", true)
        );
    }
}
