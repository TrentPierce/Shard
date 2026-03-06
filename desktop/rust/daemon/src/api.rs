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
    let now = now_ms();
    let topo = state.topology.lock().await;
    let peers = state.peers.lock().await;
    let known = state.known_peers.lock().await;
    let verified_count = peers.values().filter(|p| p.verified).count();
    let capacity = state.capacity.load(Ordering::Relaxed);
    let load = state.current_load.load(Ordering::Relaxed);
    let latency_ms = state.avg_latency_ms.load(Ordering::Relaxed);
    let browser_scout_count = {
        let mut sessions = state.browser_sessions.lock().await;
        prune_browser_sessions(&mut sessions, now);
        sessions.len()
    };
    let (
        draft_capable_scout_count,
        active_browser_runtime_count,
        scout_runtime_webgpu_total,
        scout_runtime_wasm_total,
        scout_last_submit_success_ms,
    ) = {
        let mut runtime = state.scout_client_runtime.lock().await;
        prune_scout_client_runtime(&mut runtime, now);
        summarize_scout_client_runtime(&runtime, now)
    };
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
    let scout_ingress_enabled = state.scout_ingress_enabled.load(Ordering::Relaxed);
    let scout_count = if scout_ingress_enabled {
        draft_capable_scout_count.max(recent_scout_submitters)
    } else {
        0
    };
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
        "scout_ingress_enabled": scout_ingress_enabled,
        "rust_sidecar": "connected",
        "rust_version": env!("CARGO_PKG_VERSION"),
        "rust_uptime_ms": now_ms() - state.daemon_start,
        "peer_id": topo.local_peer_id,
        "connected_peers": peers.len(),
        "verified_peers": verified_count,
        "active_scouts": scout_count,
        "draft_capable_scouts": draft_capable_scout_count,
        "active_browser_sessions": browser_scout_count.max(active_browser_runtime_count),
        "recent_scout_submitters": recent_scout_submitters,
        "scout_runtime_webgpu_total": scout_runtime_webgpu_total,
        "scout_runtime_wasm_total": scout_runtime_wasm_total,
        "scout_last_submit_success_ms": scout_last_submit_success_ms,
        "known_peers": known.len(),
        "uptime_ms": now_ms() - state.daemon_start,
        "listen_addrs": topo.listen_addrs,
        "public_api": topo.is_public,
        "private_mode": state.private_mode,
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

pub(crate) async fn scout_ingress_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "ok": true,
        "scout_ingress_enabled": state.scout_ingress_enabled.load(Ordering::Relaxed),
    }))
}

pub(crate) async fn scout_ingress_update_handler(
    AxumState(state): AxumState<SharedState>,
    headers: HeaderMap,
    Json(req): Json<ScoutIngressUpdateRequest>,
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
                "detail": "admin key required for scout ingress updates",
            }));
        }
    }

    state
        .scout_ingress_enabled
        .store(req.enabled, Ordering::Relaxed);
    if !req.enabled {
        let mut blackout = state.scout_blackout.lock().await;
        *blackout = ScoutBlackoutState::default();
    }

    Json(serde_json::json!({
        "ok": true,
        "scout_ingress_enabled": req.enabled,
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
        "private_mode": state.private_mode,
        "recent_logs": logs.iter().cloned().collect::<Vec<String>>(),
    }))
}

pub(crate) async fn node_consensus_role_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    if let Some(handle) = state.consensus.as_ref() {
        let snapshot = handle.snapshot().await;
        Json(serde_json::json!({
            "role": snapshot.role,
            "term": snapshot.term,
            "leader_id": snapshot.leader_id,
        }))
    } else {
        Json(serde_json::json!({
            "role": "disabled",
            "term": 0,
            "leader_id": serde_json::Value::Null,
        }))
    }
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
        "relay_reservation_active": topo.relay_reservation_active,
        "nat_status": topo.nat_status.clone(),
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
pub(crate) const SCOUT_CLIENT_RUNTIME_TTL_MS: u128 = 10 * 60 * 1000;
pub(crate) const SCOUT_CLIENT_ACTIVE_WINDOW_MS: u128 = 3 * 60 * 1000;
const DEFAULT_SCOUT_WORK_MAX_AGE_MS: u128 = 180_000;
const DEFAULT_SCOUT_BACKPRESSURE_START_QUEUE_DEPTH: usize = 4;
const DEFAULT_SCOUT_BACKPRESSURE_MEDIUM_QUEUE_DEPTH: usize = 8;
const DEFAULT_SCOUT_BACKPRESSURE_HIGH_QUEUE_DEPTH: usize = 12;
const DEFAULT_SCOUT_BACKPRESSURE_LATENCY_WARN_MS: u64 = 3_000;
const DEFAULT_SCOUT_BACKPRESSURE_LATENCY_SEVERE_MS: u64 = 6_000;
const DEFAULT_SCOUT_ADMISSION_QUEUE_DEPTH: usize = 5;
const DEFAULT_SCOUT_ADMISSION_QUEUE_HARD_DEPTH: usize = 10;
const DEFAULT_SCOUT_ADMISSION_LATENCY_SOFT_MS: u64 = 4_500;
const DEFAULT_SCOUT_ADMISSION_LATENCY_HARD_MS: u64 = 6_000;
const DEFAULT_SCOUT_ADMISSION_RETRY_MIN_MS: u64 = 250;
const DEFAULT_SCOUT_ADMISSION_RETRY_MAX_MS: u64 = 4_000;
const DEFAULT_SCOUT_POLL_MIN_INTERVAL_MS: u128 = 75;
const DEFAULT_SCOUT_DRAFT_MIN_INTERVAL_MS: u128 = 50;
const DEFAULT_SCOUT_RATE_LIMIT_RETENTION_MS: u128 = 10 * 60 * 1000;
const DEFAULT_SCOUT_ACTIVE_CAP: usize = 4;
const DEFAULT_SCOUT_ACTIVE_CAP_SOFT: usize = 2;
const DEFAULT_SCOUT_ACTIVE_CAP_HARD: usize = 1;
const DEFAULT_SCOUT_LEASE_TTL_MS: u128 = 12_000;
const DEFAULT_SCOUT_BLACKOUT_TRIGGER_MS: u128 = 15_000;
const DEFAULT_SCOUT_BLACKOUT_DURATION_MS: u128 = 20_000;
const DEFAULT_SCOUT_REOPEN_STAGE_MS: u128 = 12_000;
const DEFAULT_SCOUT_MIN_QUALITY_SCORE: i32 = 35;
const DEFAULT_SCOUT_MIN_QUALITY_SAMPLES: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScoutAdmissionMode {
    Allow,
    SoftBackpressure,
    HardCircuit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScoutBlackoutMode {
    Open,
    Blackout,
    ReopenStage1,
    ReopenStage2,
    ReopenStage3,
}

#[derive(Clone, Copy, Debug)]
struct ScoutAdmissionDecision {
    mode: ScoutAdmissionMode,
    retry_after_ms: u64,
}

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

pub(crate) fn prune_scout_client_runtime(
    statuses: &mut HashMap<String, ScoutClientRuntimeStatus>,
    now: u128,
) {
    statuses.retain(|_, status| {
        now.saturating_sub(status.last_event_ms) <= SCOUT_CLIENT_RUNTIME_TTL_MS
    });
}

fn summarize_scout_client_runtime(
    statuses: &HashMap<String, ScoutClientRuntimeStatus>,
    now: u128,
) -> (usize, usize, usize, usize, Option<u128>) {
    let mut draft_capable = 0usize;
    let mut active_runtime_total = 0usize;
    let mut webgpu_total = 0usize;
    let mut wasm_total = 0usize;
    let mut last_submit_success_ms: Option<u128> = None;

    for status in statuses.values() {
        let active = now.saturating_sub(status.last_event_ms) <= SCOUT_CLIENT_ACTIVE_WINDOW_MS;
        if active {
            active_runtime_total += 1;
        }
        let mode = status.runtime_mode.as_deref().unwrap_or_default();
        if mode.eq_ignore_ascii_case("webgpu") {
            webgpu_total += 1;
            if active {
                draft_capable += 1;
            }
        } else if mode.eq_ignore_ascii_case("wasm") {
            wasm_total += 1;
        }

        if let Some(ts) = status.last_submit_success_ms {
            last_submit_success_ms =
                Some(last_submit_success_ms.map_or(ts, |current| current.max(ts)));
        }
    }

    (
        draft_capable,
        active_runtime_total,
        webgpu_total,
        wasm_total,
        last_submit_success_ms,
    )
}

fn scout_backpressure_start_queue_depth() -> usize {
    std::env::var("SHARD_SCOUT_BACKPRESSURE_START_QUEUE_DEPTH")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_SCOUT_BACKPRESSURE_START_QUEUE_DEPTH)
}

fn scout_backpressure_latency_warn_ms() -> u64 {
    std::env::var("SHARD_SCOUT_BACKPRESSURE_LATENCY_WARN_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_SCOUT_BACKPRESSURE_LATENCY_WARN_MS)
}

fn scout_backpressure_latency_severe_ms() -> u64 {
    std::env::var("SHARD_SCOUT_BACKPRESSURE_LATENCY_SEVERE_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_SCOUT_BACKPRESSURE_LATENCY_SEVERE_MS)
}

fn scout_admission_queue_depth() -> usize {
    std::env::var("SHARD_SCOUT_ADMISSION_QUEUE_DEPTH")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_SCOUT_ADMISSION_QUEUE_DEPTH)
}

fn scout_admission_queue_hard_depth() -> usize {
    std::env::var("SHARD_SCOUT_ADMISSION_QUEUE_HARD_DEPTH")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_SCOUT_ADMISSION_QUEUE_HARD_DEPTH)
}

fn scout_admission_latency_soft_ms() -> u64 {
    std::env::var("SHARD_SCOUT_ADMISSION_LATENCY_SOFT_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_SCOUT_ADMISSION_LATENCY_SOFT_MS)
}

fn scout_admission_latency_hard_ms() -> u64 {
    std::env::var("SHARD_SCOUT_ADMISSION_LATENCY_HARD_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_SCOUT_ADMISSION_LATENCY_HARD_MS)
}

fn scout_poll_min_interval_ms() -> u128 {
    std::env::var("SHARD_SCOUT_POLL_MIN_INTERVAL_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u128>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_SCOUT_POLL_MIN_INTERVAL_MS)
}

fn scout_draft_min_interval_ms() -> u128 {
    std::env::var("SHARD_SCOUT_DRAFT_MIN_INTERVAL_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u128>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_SCOUT_DRAFT_MIN_INTERVAL_MS)
}

fn scout_active_cap() -> usize {
    std::env::var("SHARD_SCOUT_ACTIVE_CAP")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_SCOUT_ACTIVE_CAP)
}

fn scout_active_cap_soft() -> usize {
    std::env::var("SHARD_SCOUT_ACTIVE_CAP_SOFT")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_SCOUT_ACTIVE_CAP_SOFT)
}

fn scout_active_cap_hard() -> usize {
    std::env::var("SHARD_SCOUT_ACTIVE_CAP_HARD")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_SCOUT_ACTIVE_CAP_HARD)
}

fn scout_lease_ttl_ms() -> u128 {
    std::env::var("SHARD_SCOUT_LEASE_TTL_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u128>().ok())
        .map(|v| v.clamp(1_000, 120_000))
        .unwrap_or(DEFAULT_SCOUT_LEASE_TTL_MS)
}

fn scout_blackout_trigger_ms() -> u128 {
    std::env::var("SHARD_SCOUT_BLACKOUT_TRIGGER_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u128>().ok())
        .map(|v| v.clamp(1_000, 120_000))
        .unwrap_or(DEFAULT_SCOUT_BLACKOUT_TRIGGER_MS)
}

fn scout_blackout_duration_ms() -> u128 {
    std::env::var("SHARD_SCOUT_BLACKOUT_DURATION_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u128>().ok())
        .map(|v| v.clamp(1_000, 120_000))
        .unwrap_or(DEFAULT_SCOUT_BLACKOUT_DURATION_MS)
}

fn scout_reopen_stage_ms() -> u128 {
    std::env::var("SHARD_SCOUT_REOPEN_STAGE_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u128>().ok())
        .map(|v| v.clamp(1_000, 120_000))
        .unwrap_or(DEFAULT_SCOUT_REOPEN_STAGE_MS)
}

fn scout_min_quality_score() -> i32 {
    std::env::var("SHARD_SCOUT_MIN_QUALITY_SCORE")
        .ok()
        .and_then(|v| v.trim().parse::<i32>().ok())
        .map(|v| v.clamp(0, 100))
        .unwrap_or(DEFAULT_SCOUT_MIN_QUALITY_SCORE)
}

fn scout_min_quality_samples() -> usize {
    std::env::var("SHARD_SCOUT_MIN_QUALITY_SAMPLES")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .map(|v| v.clamp(1, 64))
        .unwrap_or(DEFAULT_SCOUT_MIN_QUALITY_SAMPLES)
}

fn verifier_in_flight_depth(state: &SharedState) -> usize {
    state.in_flight_count.load(Ordering::Relaxed)
}

fn verifier_latency_snapshot(state: &SharedState) -> (u64, u64) {
    let avg_latency_ms = state.avg_latency_ms.load(Ordering::Relaxed) as u64;
    // gossipsub p95 reflects transport propagation, not full request latency.
    // Keep it as a lower-bound hint, but never below the local request EWMA.
    let p95_latency_ms = state
        .gossipsub_latency_hist
        .percentiles()
        .p95_ms
        .max(avg_latency_ms);
    (avg_latency_ms, p95_latency_ms)
}

fn speculative_pending_max_age_ms() -> u128 {
    static MAX_AGE_MS: std::sync::OnceLock<u128> = std::sync::OnceLock::new();
    *MAX_AGE_MS.get_or_init(|| {
        std::env::var("SHARD_SPECULATIVE_PENDING_MAX_AGE_MS")
            .ok()
            .and_then(|value| value.trim().parse::<u128>().ok())
            .map(|value| value.clamp(5_000, 120_000))
            .unwrap_or(20_000)
    })
}

pub(crate) async fn prune_stale_speculative_pending(state: &SharedState, now: u128) -> usize {
    let max_age_ms = speculative_pending_max_age_ms();
    let stale_ids = {
        let pending = state.speculative_pending.lock().await;
        pending
            .iter()
            .filter_map(|(work_id, issued_at_ms)| {
                (now.saturating_sub(*issued_at_ms) > max_age_ms).then(|| work_id.clone())
            })
            .collect::<Vec<_>>()
    };

    if stale_ids.is_empty() {
        return 0;
    }

    {
        let mut pending = state.speculative_pending.lock().await;
        for work_id in &stale_ids {
            pending.remove(work_id);
        }
    }
    {
        let mut leases = state.scout_work_leases.lock().await;
        for work_id in &stale_ids {
            leases.remove(work_id);
        }
    }
    {
        let mut mailbox = state.scout_draft_mailbox.lock().await;
        for work_id in &stale_ids {
            mailbox.remove(work_id);
        }
    }
    {
        let mut by_id = state.idempotent_results.lock().await;
        for work_id in &stale_ids {
            by_id.remove(work_id);
        }
    }
    {
        let mut notifiers = state.scout_draft_notifiers.lock().await;
        for work_id in &stale_ids {
            notifiers.remove(work_id);
        }
    }

    stale_ids.len()
}

fn effective_verifier_queue_depth(state: &SharedState, pending_depth: usize) -> usize {
    pending_depth.max(verifier_in_flight_depth(state))
}

fn scout_admission_retry_after_ms(
    queue_depth: usize,
    avg_latency_ms: u64,
    p95_latency_ms: u64,
) -> u64 {
    let min_retry = DEFAULT_SCOUT_ADMISSION_RETRY_MIN_MS;
    let max_retry = DEFAULT_SCOUT_ADMISSION_RETRY_MAX_MS;
    let queue_penalty = queue_depth.saturating_mul(120) as u64;
    let lat_penalty = ((avg_latency_ms.max(p95_latency_ms)) / 8).min(3_000);
    min_retry
        .saturating_add(queue_penalty)
        .saturating_add(lat_penalty)
        .clamp(min_retry, max_retry)
}

fn scout_admission_decision(
    queue_depth: usize,
    avg_latency_ms: u64,
    p95_latency_ms: u64,
) -> ScoutAdmissionDecision {
    let soft_queue = scout_admission_queue_depth();
    let hard_queue = scout_admission_queue_hard_depth().max(soft_queue);
    let soft_latency = scout_admission_latency_soft_ms();
    let hard_latency = scout_admission_latency_hard_ms().max(soft_latency);

    let mode = if queue_depth >= hard_queue
        || avg_latency_ms >= hard_latency
        || p95_latency_ms >= hard_latency
    {
        ScoutAdmissionMode::HardCircuit
    } else if queue_depth >= soft_queue
        || avg_latency_ms >= soft_latency
        || p95_latency_ms >= soft_latency
    {
        ScoutAdmissionMode::SoftBackpressure
    } else {
        ScoutAdmissionMode::Allow
    };
    ScoutAdmissionDecision {
        mode,
        retry_after_ms: scout_admission_retry_after_ms(queue_depth, avg_latency_ms, p95_latency_ms),
    }
}

fn deterministic_sample_bucket(scout_id: &str, window: u128, modulus: u64) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    scout_id.hash(&mut hasher);
    window.hash(&mut hasher);
    let m = modulus.max(1);
    hasher.finish() % m
}

fn scout_active_cap_for_mode(mode: ScoutAdmissionMode) -> usize {
    let base = scout_active_cap();
    let soft = scout_active_cap_soft().min(base.max(1));
    let hard = scout_active_cap_hard().min(soft.max(1));
    match mode {
        ScoutAdmissionMode::Allow => base,
        ScoutAdmissionMode::SoftBackpressure => soft,
        ScoutAdmissionMode::HardCircuit => hard,
    }
}

fn scout_blackout_mode(state: &ScoutBlackoutState, now: u128) -> ScoutBlackoutMode {
    if now < state.blackout_until_ms {
        return ScoutBlackoutMode::Blackout;
    }
    if let Some(reopen_started_ms) = state.reopen_started_ms {
        let stage_ms = scout_reopen_stage_ms();
        let elapsed = now.saturating_sub(reopen_started_ms);
        if elapsed < stage_ms {
            return ScoutBlackoutMode::ReopenStage1;
        }
        if elapsed < stage_ms.saturating_mul(2) {
            return ScoutBlackoutMode::ReopenStage2;
        }
        if elapsed < stage_ms.saturating_mul(3) {
            return ScoutBlackoutMode::ReopenStage3;
        }
    }
    ScoutBlackoutMode::Open
}

fn scout_active_cap_for_blackout(mode: ScoutBlackoutMode, cap: usize) -> usize {
    match mode {
        ScoutBlackoutMode::Open => cap,
        ScoutBlackoutMode::Blackout => 0,
        ScoutBlackoutMode::ReopenStage1 => cap.min(1),
        ScoutBlackoutMode::ReopenStage2 => cap.min(2),
        ScoutBlackoutMode::ReopenStage3 => cap.min((cap.max(2) + 1) / 2),
    }
}

async fn update_scout_blackout_state(
    state: &SharedState,
    queue_depth: usize,
    p95_latency_ms: u64,
    now: u128,
) -> ScoutBlackoutMode {
    let mut snapshot = state.scout_blackout.lock().await;
    let hard_queue = scout_admission_queue_hard_depth();
    let hard_latency = scout_admission_latency_hard_ms();
    let overloaded = queue_depth >= hard_queue || p95_latency_ms >= hard_latency;

    if now < snapshot.blackout_until_ms {
        if overloaded {
            // Keep blackout active while overload persists to avoid immediate thrash.
            snapshot.blackout_until_ms = now.saturating_add(scout_blackout_duration_ms());
        }
        return ScoutBlackoutMode::Blackout;
    }

    if overloaded {
        match snapshot.overload_since_ms {
            Some(since) if now.saturating_sub(since) >= scout_blackout_trigger_ms() => {
                snapshot.blackout_until_ms = now.saturating_add(scout_blackout_duration_ms());
                snapshot.overload_since_ms = None;
                snapshot.reopen_started_ms = None;
                drop(snapshot);
                state.system_metrics.inc_scout_blackout_enter();
                return ScoutBlackoutMode::Blackout;
            }
            Some(_) => {}
            None => {
                snapshot.overload_since_ms = Some(now);
            }
        }
    } else {
        snapshot.overload_since_ms = None;
    }

    if snapshot.blackout_until_ms > 0 {
        if snapshot.reopen_started_ms.is_none() {
            snapshot.reopen_started_ms = Some(now);
            drop(snapshot);
            state.system_metrics.inc_scout_blackout_exit();
            return ScoutBlackoutMode::ReopenStage1;
        }
        let mode = scout_blackout_mode(&*snapshot, now);
        if mode == ScoutBlackoutMode::Open {
            snapshot.blackout_until_ms = 0;
            snapshot.reopen_started_ms = None;
        }
        return mode;
    }

    ScoutBlackoutMode::Open
}

fn prune_expired_scout_leases(leases: &mut HashMap<String, ScoutWorkLease>, now: u128) -> usize {
    let before = leases.len();
    leases.retain(|_, lease| now < lease.expires_at_ms);
    before.saturating_sub(leases.len())
}

fn prune_stale_scout_work_queue(
    queue: &mut std::collections::VecDeque<WorkRequest>,
    now: u128,
) -> usize {
    let max_age_ms = scout_work_max_age_ms();
    let before = queue.len();
    queue.retain(|work| {
        let created_at_ms = work.created_at_ms.unwrap_or(now);
        now.saturating_sub(created_at_ms) <= max_age_ms
    });
    before.saturating_sub(queue.len())
}

async fn prune_stale_scout_work_queue_for_state(state: &SharedState, now: u128) {
    let mut queue = state.scout_work.lock().await;
    let removed = prune_stale_scout_work_queue(&mut queue, now);
    if removed > 0 {
        tracing::debug!(
            removed,
            remaining = queue.len(),
            "pruned stale scout work items"
        );
    }
}

async fn prune_expired_scout_leases_for_state(state: &SharedState, now: u128) {
    let expired = {
        let mut leases = state.scout_work_leases.lock().await;
        prune_expired_scout_leases(&mut leases, now)
    };
    for _ in 0..expired {
        state.system_metrics.inc_scout_work_lease_expired();
    }
}

fn apply_scout_rate_limit(
    table: &mut HashMap<String, u128>,
    scout_id: &str,
    now: u128,
    min_interval_ms: u128,
) -> Option<u64> {
    table.retain(|_, ts| now.saturating_sub(*ts) <= DEFAULT_SCOUT_RATE_LIMIT_RETENTION_MS);
    if min_interval_ms == 0 {
        table.insert(scout_id.to_string(), now);
        return None;
    }
    if let Some(last_seen) = table.get(scout_id) {
        let elapsed = now.saturating_sub(*last_seen);
        if elapsed < min_interval_ms {
            let retry = min_interval_ms.saturating_sub(elapsed) as u64;
            return Some(retry.max(1));
        }
    }
    table.insert(scout_id.to_string(), now);
    None
}

async fn recent_active_scouts(state: &SharedState, now: u128) -> std::collections::HashSet<String> {
    let cutoff = now.saturating_sub(SCOUT_CLIENT_ACTIVE_WINDOW_MS);
    let results = state.results.lock().await;
    results
        .iter()
        .filter_map(|entry| {
            let ts = entry.created_at_ms.unwrap_or(0);
            if ts >= cutoff {
                Some(entry.peer_id.clone())
            } else {
                None
            }
        })
        .collect()
}

fn scout_assignment_backpressured(
    scout_id: &str,
    queue_depth: usize,
    avg_latency_ms: u64,
    now: u128,
) -> bool {
    let start_depth = scout_backpressure_start_queue_depth();
    let latency_warn_ms = scout_backpressure_latency_warn_ms();
    let latency_severe_ms = scout_backpressure_latency_severe_ms();
    if queue_depth < start_depth && avg_latency_ms < latency_warn_ms {
        return false;
    }

    let modulus = if queue_depth >= DEFAULT_SCOUT_BACKPRESSURE_HIGH_QUEUE_DEPTH
        || avg_latency_ms >= latency_severe_ms
    {
        8
    } else if queue_depth >= DEFAULT_SCOUT_BACKPRESSURE_MEDIUM_QUEUE_DEPTH
        || avg_latency_ms >= latency_warn_ms
    {
        6
    } else {
        4
    };
    // Windowing keeps assignment deterministic for a short interval and avoids stampedes.
    let window = now / 1_000;
    deterministic_sample_bucket(scout_id, window, modulus) != 0
}

fn round_pct(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn pct(part: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        round_pct((part as f64 / total as f64) * 100.0)
    }
}

impl WebGPUStats {
    pub(crate) fn record_probe(&mut self, probe: &WebGPUProbeResult) {
        self.total_probes = self.total_probes.saturating_add(1);

        let browser = probe.browser.trim();
        if !browser.is_empty() {
            *self.browser_counts.entry(browser.to_string()).or_insert(0) += 1;
        }
        let os = probe.os.trim();
        if !os.is_empty() {
            *self.os_counts.entry(os.to_string()).or_insert(0) += 1;
        }

        if probe.eligible {
            self.eligible = self.eligible.saturating_add(1);
            let tier = probe.tier.to_ascii_lowercase();
            if tier == "high-performance" {
                self.high_performance = self.high_performance.saturating_add(1);
            } else if tier == "low-power" {
                self.low_power = self.low_power.saturating_add(1);
            }
        } else {
            let reason = probe
                .reason
                .as_ref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .unwrap_or("unknown");
            *self
                .ineligible_reasons
                .entry(reason.to_string())
                .or_insert(0) += 1;
        }
    }

    pub(crate) fn coverage_summary(&self) -> serde_json::Value {
        let ineligible = self.total_probes.saturating_sub(self.eligible);
        let mut reason_map = serde_json::Map::new();
        for (key, count) in &self.ineligible_reasons {
            reason_map.insert(
                key.clone(),
                serde_json::json!(pct(*count, self.total_probes)),
            );
        }
        let mut browser_map = serde_json::Map::new();
        for (key, count) in &self.browser_counts {
            browser_map.insert(
                key.clone(),
                serde_json::json!(pct(*count, self.total_probes)),
            );
        }
        let mut os_map = serde_json::Map::new();
        for (key, count) in &self.os_counts {
            os_map.insert(
                key.clone(),
                serde_json::json!(pct(*count, self.total_probes)),
            );
        }

        serde_json::json!({
            "total_probes": self.total_probes,
            "eligible_pct": pct(self.eligible, self.total_probes),
            "high_performance_pct": pct(self.high_performance, self.total_probes),
            "low_power_pct": pct(self.low_power, self.total_probes),
            "ineligible_pct": pct(ineligible, self.total_probes),
            "ineligible_reason_breakdown": reason_map,
            "browser_breakdown": browser_map,
            "os_breakdown": os_map,
        })
    }
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
    let encoded = match wire.encode() {
        Ok(encoded) => encoded,
        Err(detail) => {
            return Json(serde_json::json!({
                "ok": false,
                "detail": format!("failed to encode tensor wire payload: {detail}"),
            }))
        }
    };
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

    let now = now_ms();
    let event_name = event.event.as_str();

    match event.event.as_str() {
        "submit_attempt" => state.system_metrics.inc_scout_client_submit_attempt(),
        "submit_success" => {
            state.system_metrics.inc_scout_client_submit_success();
        }
        "submit_http_error" => state.system_metrics.inc_scout_client_submit_http_failure(),
        "submit_timeout" => state.system_metrics.inc_scout_client_submit_timeout(),
        "submit_pow_failure" => state.system_metrics.inc_scout_client_submit_pow_failure(),
        "submit_network_error" => state
            .system_metrics
            .inc_scout_client_submit_network_failure(),
        "generate_failure" => state.system_metrics.inc_scout_client_generate_failure(),
        "fallback_draft_used" => state.system_metrics.inc_scout_client_fallback_draft(),
        "runtime_webgpu_ready" | "runtime_wasm_fallback" => {}
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

    {
        let mut runtime = state.scout_client_runtime.lock().await;
        prune_scout_client_runtime(&mut runtime, now);
        let entry =
            runtime
                .entry(scout_id.to_string())
                .or_insert_with(|| ScoutClientRuntimeStatus {
                    scout_id: scout_id.to_string(),
                    runtime_mode: None,
                    last_event: event_name.to_string(),
                    last_event_detail: None,
                    last_event_ms: now,
                    last_submit_success_ms: None,
                });
        entry.last_event = event_name.to_string();
        entry.last_event_detail = event.detail.as_ref().map(|d| d.chars().take(300).collect());
        entry.last_event_ms = now;
        if event_name == "runtime_webgpu_ready" {
            entry.runtime_mode = Some("webgpu".to_string());
        } else if event_name == "runtime_wasm_fallback" {
            entry.runtime_mode = Some("wasm".to_string());
        }
        if event_name == "submit_success" {
            entry.last_submit_success_ms = Some(now);
        }
    }

    Json(serde_json::json!({ "ok": true }))
}

pub(crate) async fn webgpu_telemetry_handler(
    AxumState(state): AxumState<SharedState>,
    Json(probe): Json<WebGPUProbeResult>,
) -> Json<serde_json::Value> {
    let tier = probe.tier.to_ascii_lowercase();
    let tier_valid = matches!(tier.as_str(), "high-performance" | "low-power" | "none");
    if !tier_valid {
        return Json(serde_json::json!({
            "ok": false,
            "detail": "tier must be one of: high-performance, low-power, none",
        }));
    }

    if !probe.eligible
        && probe
            .reason
            .as_ref()
            .map(|r| r.trim().is_empty())
            .unwrap_or(true)
    {
        return Json(serde_json::json!({
            "ok": false,
            "detail": "reason is required for ineligible probes",
        }));
    }

    let mut stats = state.webgpu_stats.lock().await;
    stats.record_probe(&probe);
    Json(serde_json::json!({ "ok": true }))
}

pub(crate) async fn webgpu_coverage_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let stats = state.webgpu_stats.lock().await;
    Json(stats.coverage_summary())
}

fn scout_config_snapshot_json() -> serde_json::Value {
    serde_json::json!({
        "profile": std::env::var("SHARD_RELEASE_PROFILE").unwrap_or_else(|_| "default".to_string()),
        "scout_work_max_age_ms": scout_work_max_age_ms(),
        "scout_client_runtime_ttl_ms": SCOUT_CLIENT_RUNTIME_TTL_MS,
        "scout_client_active_window_ms": SCOUT_CLIENT_ACTIVE_WINDOW_MS,
        "backpressure": {
            "start_queue_depth": scout_backpressure_start_queue_depth(),
            "medium_queue_depth": DEFAULT_SCOUT_BACKPRESSURE_MEDIUM_QUEUE_DEPTH,
            "high_queue_depth": DEFAULT_SCOUT_BACKPRESSURE_HIGH_QUEUE_DEPTH,
            "latency_warn_ms": scout_backpressure_latency_warn_ms(),
            "latency_severe_ms": scout_backpressure_latency_severe_ms(),
        },
        "admission": {
            "queue_depth_soft": scout_admission_queue_depth(),
            "queue_depth_hard": scout_admission_queue_hard_depth(),
            "latency_soft_ms": scout_admission_latency_soft_ms(),
            "latency_hard_ms": scout_admission_latency_hard_ms(),
            "retry_min_ms": DEFAULT_SCOUT_ADMISSION_RETRY_MIN_MS,
            "retry_max_ms": DEFAULT_SCOUT_ADMISSION_RETRY_MAX_MS,
        },
        "rate_limit": {
            "poll_min_interval_ms": scout_poll_min_interval_ms(),
            "draft_min_interval_ms": scout_draft_min_interval_ms(),
            "retention_ms": DEFAULT_SCOUT_RATE_LIMIT_RETENTION_MS,
        },
        "active_cap": {
            "base": scout_active_cap(),
            "soft": scout_active_cap_soft(),
            "hard": scout_active_cap_hard(),
        },
        "lease": {
            "ttl_ms": scout_lease_ttl_ms(),
        },
        "blackout": {
            "trigger_ms": scout_blackout_trigger_ms(),
            "duration_ms": scout_blackout_duration_ms(),
            "reopen_stage_ms": scout_reopen_stage_ms(),
        },
        "quality": {
            "min_score": scout_min_quality_score(),
            "min_samples": scout_min_quality_samples(),
        },
    })
}

pub(crate) async fn scout_config_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let now = now_ms();
    prune_stale_scout_work_queue_for_state(&state, now).await;
    let pending_queue_depth = state.speculative_pending.lock().await.len();
    let queue_depth = effective_verifier_queue_depth(&state, pending_queue_depth);
    let (avg_latency_ms, p95_latency_ms) = verifier_latency_snapshot(&state);
    let admission = scout_admission_decision(queue_depth, avg_latency_ms, p95_latency_ms);
    let blackout_mode = update_scout_blackout_state(&state, queue_depth, p95_latency_ms, now).await;
    let active_scouts = recent_active_scouts(&state, now).await;
    let active_cap =
        scout_active_cap_for_blackout(blackout_mode, scout_active_cap_for_mode(admission.mode));
    let lease_count = state.scout_work_leases.lock().await.len();
    Json(serde_json::json!({
        "ok": true,
        "config": scout_config_snapshot_json(),
        "runtime": {
            "queue_depth": queue_depth,
            "average_latency_ms": avg_latency_ms,
            "p95_latency_ms": p95_latency_ms,
            "admission_mode": match admission.mode {
                ScoutAdmissionMode::Allow => "allow",
                ScoutAdmissionMode::SoftBackpressure => "soft_backpressure",
                ScoutAdmissionMode::HardCircuit => "hard_circuit",
            },
            "retry_after_ms": admission.retry_after_ms,
            "active_scouts_recent": active_scouts.len(),
            "active_cap_current": active_cap,
            "blackout_mode": match blackout_mode {
                ScoutBlackoutMode::Open => "open",
                ScoutBlackoutMode::Blackout => "blackout",
                ScoutBlackoutMode::ReopenStage1 => "reopen_stage_1",
                ScoutBlackoutMode::ReopenStage2 => "reopen_stage_2",
                ScoutBlackoutMode::ReopenStage3 => "reopen_stage_3",
            },
            "active_leases": lease_count,
        }
    }))
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
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    if !state.scout_ingress_enabled.load(Ordering::Relaxed) {
        return Err((
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "work": null,
                "transient_error": true,
                "detail": "scout_ingress_disabled",
                "retry_after_ms": 1000,
            })),
        ));
    }
    let scout_id = match require_scout_id(&query) {
        Ok(value) => value,
        Err(response) => return Err((axum::http::StatusCode::BAD_REQUEST, response)),
    };
    if let Err(response) = ensure_pow_verified(&state, scout_id).await {
        return Err((axum::http::StatusCode::FORBIDDEN, response));
    }
    state.system_metrics.inc_scout_work_poll();
    let now = now_ms();
    prune_stale_speculative_pending(&state, now).await;
    prune_expired_scout_leases_for_state(&state, now).await;
    prune_stale_scout_work_queue_for_state(&state, now).await;
    {
        let mut polls = state.scout_work_last_poll.lock().await;
        if let Some(retry_after_ms) =
            apply_scout_rate_limit(&mut polls, scout_id, now, scout_poll_min_interval_ms())
        {
            state.system_metrics.inc_scout_work_rate_limited();
            return Err((
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({
                    "work": null,
                    "transient_error": true,
                    "detail": "scout_poll_rate_limited",
                    "retry_after_ms": retry_after_ms,
                })),
            ));
        }
    }
    {
        let mut penalties = state.scout_penalties.lock().await;
        if penalties.is_blackholed(scout_id) {
            return Err((
                axum::http::StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "work": null,
                    "transient_error": true,
                    "detail": "scout_blackholed",
                    "retry_after_ms": scout_blackout_duration_ms().min(5_000) as u64,
                })),
            ));
        }
        if let Some((score, samples)) = penalties.quality_snapshot(scout_id) {
            if samples >= scout_min_quality_samples() && score < scout_min_quality_score() {
                return Err((
                    axum::http::StatusCode::TOO_MANY_REQUESTS,
                    Json(serde_json::json!({
                        "work": null,
                        "transient_error": true,
                        "detail": "scout_quality_backoff",
                        "quality_score": score,
                        "quality_samples": samples,
                        "retry_after_ms": 1000,
                    })),
                ));
            }
        }
    }
    let pending_queue_depth = state.speculative_pending.lock().await.len();
    let queue_depth = effective_verifier_queue_depth(&state, pending_queue_depth);
    let (avg_latency_ms, p95_latency_ms) = verifier_latency_snapshot(&state);
    let blackout_mode = update_scout_blackout_state(&state, queue_depth, p95_latency_ms, now).await;
    if blackout_mode == ScoutBlackoutMode::Blackout {
        state.system_metrics.inc_scout_work_overload_reject();
        state.system_metrics.inc_scout_work_empty_poll();
        return Err((
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "work": null,
                "transient_error": true,
                "detail": "scout_blackout_active",
                "queue_depth": queue_depth,
                "p95_latency_ms": p95_latency_ms,
                "retry_after_ms": scout_blackout_duration_ms().min(u64::MAX as u128) as u64,
            })),
        ));
    }
    let admission = scout_admission_decision(queue_depth, avg_latency_ms, p95_latency_ms);
    if admission.mode != ScoutAdmissionMode::Allow {
        state.system_metrics.inc_scout_work_overload_reject();
        state.system_metrics.inc_scout_work_empty_poll();
        let detail = if admission.mode == ScoutAdmissionMode::HardCircuit {
            "scout_circuit_open"
        } else {
            "scout_backpressure_active"
        };
        tracing::debug!(
            scout_id = %scout_id,
            queue_depth,
            avg_latency_ms,
            p95_latency_ms,
            retry_after_ms = admission.retry_after_ms,
            detail,
            "rejecting scout work poll due to verifier load"
        );
        return Err((
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "work": null,
                "transient_error": true,
                "detail": detail,
                "queue_depth": queue_depth,
                "average_latency_ms": avg_latency_ms,
                "p95_latency_ms": p95_latency_ms,
                "retry_after_ms": admission.retry_after_ms,
            })),
        ));
    }
    let active_scouts = recent_active_scouts(&state, now).await;
    let active_cap =
        scout_active_cap_for_blackout(blackout_mode, scout_active_cap_for_mode(admission.mode));
    if active_cap == 0 {
        state.system_metrics.inc_scout_work_overload_reject();
        return Err((
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "work": null,
                "transient_error": true,
                "detail": "scout_reopen_gate_active",
                "retry_after_ms": 500,
            })),
        ));
    }
    if active_cap > 0 && active_scouts.len() >= active_cap && !active_scouts.contains(scout_id) {
        state.system_metrics.inc_scout_work_active_cap_reject();
        return Err((
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "work": null,
                "transient_error": true,
                "detail": "scout_active_cap_reached",
                "active_scouts": active_scouts.len(),
                "active_cap": active_cap,
                "retry_after_ms": admission.retry_after_ms.max(250),
            })),
        ));
    }
    if scout_assignment_backpressured(scout_id, queue_depth, avg_latency_ms, now) {
        state.system_metrics.inc_scout_work_empty_poll();
        tracing::debug!(
            scout_id = %scout_id,
            queue_depth,
            avg_latency_ms,
            "backpressure active for scout assignment; returning empty work"
        );
        return Ok(Json(serde_json::json!({
            "work": null,
            "transient_error": true,
            "detail": "backpressure_active",
            "queue_depth": queue_depth,
            "retry_after_ms": 250,
        })));
    }
    let mut queue = state.scout_work.lock().await;
    let max_age_ms = scout_work_max_age_ms();
    while let Some(mut work) = queue.pop_front() {
        let created_at_ms = work.created_at_ms.unwrap_or(now);
        let age_ms = now.saturating_sub(created_at_ms);
        if age_ms <= max_age_ms {
            let lease_id = uuid::Uuid::new_v4().to_string();
            let lease_expires_at_ms = now.saturating_add(scout_lease_ttl_ms());
            work.lease_id = Some(lease_id.clone());
            work.lease_expires_at_ms = Some(lease_expires_at_ms);
            work.assigned_scout_id = Some(scout_id.to_string());

            {
                let mut leases = state.scout_work_leases.lock().await;
                leases.insert(
                    work.request_id.clone(),
                    ScoutWorkLease {
                        lease_id,
                        scout_id: scout_id.to_string(),
                        expires_at_ms: lease_expires_at_ms,
                    },
                );
            }
            state.system_metrics.inc_scout_work_lease_issued();
            state.system_metrics.inc_scout_work_assignment();
            tracing::debug!(
                request_id = %work.request_id,
                age_ms,
                queue_depth = queue.len(),
                "assigned scout work item"
            );
            return Ok(Json(serde_json::json!({ "work": work })));
        }
        tracing::debug!(
            request_id = %work.request_id,
            age_ms,
            max_age_ms,
            "dropping stale scout work item"
        );
    }
    state.system_metrics.inc_scout_work_empty_poll();
    Ok(Json(serde_json::json!({ "work": null })))
}

pub(crate) async fn submit_draft_handler(
    AxumState(state): AxumState<SharedState>,
    Json(submission): Json<DraftResultSubmission>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    process_draft_submission(&state, submission).await
}

pub(crate) async fn signed_submit_draft_handler(
    AxumState(state): AxumState<SharedState>,
    Json(req): Json<SignedRequest<DraftResultSubmission>>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let signer = req.envelope.signer_pubkey_hex.clone();
    if let Err(detail) = req.envelope.verify() {
        record_signature_alert(&state, false).await;
        state.system_metrics.inc_signature_verification_failures();
        mark_node_failure(&state, &signer).await;
        return Ok(Json(serde_json::json!({ "ok": false, "detail": detail })));
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
        return Ok(Json(
            serde_json::json!({ "ok": false, "detail": "stale or replayed nonce" }),
        ));
    }
    record_signature_alert(&state, true).await;
    process_draft_submission(&state, req.envelope.payload).await
}

pub(crate) async fn process_draft_submission(
    state: &SharedState,
    mut submission: DraftResultSubmission,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    if !state.scout_ingress_enabled.load(Ordering::Relaxed) {
        return Err((
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "ok": false,
                "detail": "scout_ingress_disabled",
                "retry_after_ms": 1000,
            })),
        ));
    }
    submission.work_id = submission.work_id.trim().to_string();
    submission.scout_id = submission.scout_id.trim().to_string();
    submission.lease_id = submission
        .lease_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    state.system_metrics.inc_scout_draft_submission();
    let now = now_ms();
    prune_stale_speculative_pending(state, now).await;
    prune_expired_scout_leases_for_state(state, now).await;
    prune_stale_scout_work_queue_for_state(state, now).await;
    {
        let mut drafts = state.scout_draft_last_submit.lock().await;
        if let Some(retry_after_ms) = apply_scout_rate_limit(
            &mut drafts,
            submission.scout_id.as_str(),
            now,
            scout_draft_min_interval_ms(),
        ) {
            state.system_metrics.inc_scout_draft_rate_limited();
            return Err((
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({
                    "ok": false,
                    "transient_error": true,
                    "detail": "scout_draft_rate_limited",
                    "retry_after_ms": retry_after_ms,
                })),
            ));
        }
    }
    let scout_queue_depth = state.scout_work.lock().await.len();
    let pending_queue_depth = state.speculative_pending.lock().await.len();
    let effective_queue_depth = effective_verifier_queue_depth(state, pending_queue_depth);
    let (avg_latency_ms, p95_latency_ms) = verifier_latency_snapshot(state);
    let blackout_mode =
        update_scout_blackout_state(state, effective_queue_depth, p95_latency_ms, now).await;
    if blackout_mode == ScoutBlackoutMode::Blackout {
        state.system_metrics.inc_scout_draft_overload_reject();
        return Err((
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "ok": false,
                "transient_error": true,
                "detail": "scout_draft_blackout_active",
                "queue_depth": effective_queue_depth,
                "p95_latency_ms": p95_latency_ms,
                "retry_after_ms": scout_blackout_duration_ms().min(u64::MAX as u128) as u64,
            })),
        ));
    }
    let admission = scout_admission_decision(effective_queue_depth, avg_latency_ms, p95_latency_ms);
    if admission.mode != ScoutAdmissionMode::Allow {
        state.system_metrics.inc_scout_draft_overload_reject();
        let detail = if admission.mode == ScoutAdmissionMode::HardCircuit {
            "scout_draft_circuit_open"
        } else {
            "scout_draft_backpressure_active"
        };
        tracing::debug!(
            scout_id = %submission.scout_id,
            work_id = %submission.work_id,
            scout_queue_depth,
            pending_queue_depth,
            avg_latency_ms,
            p95_latency_ms,
            retry_after_ms = admission.retry_after_ms,
            detail,
            "rejecting scout draft due to verifier load"
        );
        return Err((
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "ok": false,
                "transient_error": true,
                "detail": detail,
                "queue_depth": effective_queue_depth,
                "average_latency_ms": avg_latency_ms,
                "p95_latency_ms": p95_latency_ms,
                "retry_after_ms": admission.retry_after_ms,
            })),
        ));
    }
    let pending_age_ms = {
        let pending = state.speculative_pending.lock().await;
        pending
            .get(submission.work_id.as_str())
            .copied()
            .map(|issued_at| now.saturating_sub(issued_at) as u64)
    };
    if pending_age_ms.is_none() {
        state
            .system_metrics
            .inc_speculative_wait_mismatched_work_id();
        tracing::warn!(
            work_id = %submission.work_id,
            scout_id = %submission.scout_id,
            "received scout draft for non-pending work_id"
        );
    } else if let Some(age_ms) = pending_age_ms {
        tracing::debug!(
            work_id = %submission.work_id,
            scout_id = %submission.scout_id,
            age_ms,
            "received scout draft for pending work_id"
        );
    }
    if submission.work_id.trim().is_empty() || submission.scout_id.trim().is_empty() {
        state.system_metrics.inc_task_failures();
        state
            .system_metrics
            .inc_scout_draft_reject_missing_identity();
        if !submission.scout_id.trim().is_empty() {
            mark_node_failure(state, submission.scout_id.as_str()).await;
        }
        return Ok(Json(serde_json::json!({
            "ok": false,
            "detail": "work_id and scout_id are required",
        })));
    }
    let duplicate_submission = {
        let by_id = state.idempotent_results.lock().await;
        by_id.contains_key(submission.work_id.as_str())
    };
    if duplicate_submission {
        state.system_metrics.inc_scout_draft_duplicate();
        mark_node_success(state, submission.scout_id.as_str(), 0.0).await;
        return Ok(Json(serde_json::json!({
            "ok": true,
            "detail": "duplicate draft ignored (idempotent)",
        })));
    }
    let pending_exists = {
        let pending = state.speculative_pending.lock().await;
        pending.contains_key(submission.work_id.as_str())
    };
    let lease_id = submission.lease_id.clone().unwrap_or_default();
    let lease_validation_error = {
        let mut leases = state.scout_work_leases.lock().await;
        match leases.get(submission.work_id.as_str()) {
            Some(lease) => {
                if now >= lease.expires_at_ms {
                    leases.remove(submission.work_id.as_str());
                    Some((
                        "scout_lease_expired",
                        axum::http::StatusCode::GONE,
                        true,
                        "expired",
                    ))
                } else if lease_id.is_empty() {
                    Some((
                        "scout_lease_required",
                        axum::http::StatusCode::BAD_REQUEST,
                        false,
                        "missing",
                    ))
                } else if lease.lease_id != lease_id {
                    Some((
                        "scout_lease_mismatch",
                        axum::http::StatusCode::FORBIDDEN,
                        false,
                        "mismatch",
                    ))
                } else if lease.scout_id != submission.scout_id {
                    Some((
                        "scout_lease_wrong_scout",
                        axum::http::StatusCode::FORBIDDEN,
                        false,
                        "mismatch",
                    ))
                } else {
                    None
                }
            }
            None => {
                if pending_exists {
                    Some((
                        "scout_lease_not_found",
                        axum::http::StatusCode::CONFLICT,
                        false,
                        "missing",
                    ))
                } else {
                    None
                }
            }
        }
    };
    if let Some((detail, status, transient_error, kind)) = lease_validation_error {
        state.system_metrics.inc_task_failures();
        match kind {
            "missing" => state.system_metrics.inc_scout_draft_reject_lease_missing(),
            "expired" => {
                state.system_metrics.inc_scout_draft_reject_lease_expired();
                state.system_metrics.inc_scout_work_lease_expired();
            }
            _ => state.system_metrics.inc_scout_draft_reject_lease_mismatch(),
        }
        mark_node_failure(state, submission.scout_id.as_str()).await;
        return Err((
            status,
            Json(serde_json::json!({
                "ok": false,
                "transient_error": transient_error,
                "detail": detail,
            })),
        ));
    }
    if let Err(response) = ensure_pow_verified(state, submission.scout_id.as_str()).await {
        state.system_metrics.inc_task_failures();
        state.system_metrics.inc_scout_draft_reject_pow();
        mark_node_failure(state, submission.scout_id.as_str()).await;
        return Err((axum::http::StatusCode::FORBIDDEN, response));
    }
    if let Some(spot_check) = submission.spot_check.as_ref() {
        if let Err(detail) = verify_spot_check_submission(spot_check) {
            state.system_metrics.inc_task_failures();
            state.system_metrics.inc_scout_draft_reject_spotcheck();
            mark_node_failure(state, submission.scout_id.as_str()).await;
            return Ok(Json(serde_json::json!({
                "ok": false,
                "detail": detail,
            })));
        }
    }

    submission.draft_text = sanitize_scout_draft_text(submission.draft_text.as_str());

    // Detect garbage echo-back drafts: if the draft text is a verbatim suffix of
    // the prompt context, it means the scout just echoed back the prompt tail
    // (WASM fallback or uninitialized engine). These always fail verification.
    if let Some(ref prompt_ctx) = submission.prompt_context {
        let clean_prompt = prompt_ctx.replace('\0', "").trim().to_string();
        let clean_draft = submission.draft_text.trim();
        if !clean_prompt.is_empty()
            && !clean_draft.is_empty()
            && clean_prompt.ends_with(clean_draft)
        {
            tracing::warn!(
                work_id = %submission.work_id,
                scout_id = %submission.scout_id,
                draft_len = clean_draft.len(),
                "rejecting echo-back garbage draft (draft is a suffix of prompt)"
            );
            state.system_metrics.inc_task_failures();
            mark_node_failure(state, submission.scout_id.as_str()).await;
            return Ok(Json(serde_json::json!({
                "ok": false,
                "detail": "draft rejected: echo-back of prompt tail detected",
            })));
        }
    }

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
        return Ok(Json(serde_json::json!({
            "ok": false,
            "detail": "draft_tokens required or server tokenization must produce non-empty tokens",
        })));
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

    let duplicate_after_processing = {
        let mut by_id = state.idempotent_results.lock().await;
        if by_id.contains_key(response.request_id.as_str()) {
            true
        } else {
            by_id.insert(response.request_id.clone(), response.clone());
            false
        }
    };
    if duplicate_after_processing {
        state.system_metrics.inc_scout_draft_duplicate();
        mark_node_success(state, response.peer_id.as_str(), 0.0).await;
        return Ok(Json(serde_json::json!({
            "ok": true,
            "detail": "duplicate draft ignored (idempotent)",
        })));
    }
    {
        let mut leases = state.scout_work_leases.lock().await;
        leases.remove(response.request_id.as_str());
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
            Ok(Json(
                serde_json::json!({ "ok": true, "detail": "draft queued" }),
            ))
        }
        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
            state
                .system_metrics
                .inc_scout_draft_channel_enqueue_failure();
            tracing::warn!("scout draft channel full; using mailbox fallback");
            Ok(Json(serde_json::json!({
                "ok": true,
                "detail": "draft queued (mailbox fallback; channel saturated)",
            })))
        }
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
            state
                .system_metrics
                .inc_scout_draft_channel_enqueue_failure();
            tracing::warn!("scout draft channel closed; using mailbox fallback");
            Ok(Json(serde_json::json!({
                "ok": true,
                "detail": "draft queued (mailbox fallback; channel unavailable)",
            })))
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
    let prompt_raw = submission.prompt_context.as_deref()?;
    if prompt_raw.trim().is_empty() {
        return None;
    }
    // Preserve whitespace exactly as provided to the scout.
    // Trimming here changes token boundaries and can force systematic
    // first-token mismatches during verifier comparison.
    let prompt = prompt_raw.replace('\0', "");
    if prompt.is_empty() {
        return None;
    }

    let mut prompt_tokens = engine.tokenize(prompt.as_str(), 4096).ok()?;
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
        lease_id: None,
        lease_expires_at_ms: None,
        assigned_scout_id: None,
        preferred_endpoint: None,
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

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ChatMessage {
    pub(crate) role: String,
    pub(crate) content: String,
}

#[derive(Debug, Serialize, Deserialize)]
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
    let now = now_ms();
    prune_stale_scout_work_queue_for_state(&state, now).await;
    let pending_queue_depth = state.speculative_pending.lock().await.len();
    let queue_depth = effective_verifier_queue_depth(&state, pending_queue_depth);
    let active_node_count = state
        .node_metric_reports
        .lock()
        .await
        .values()
        .filter(|snapshot| !snapshot.role.eq_ignore_ascii_case("scout"))
        .count();
    let node_latency_ms = state.avg_latency_ms.load(Ordering::Relaxed);
    let p = state.gossipsub_latency_hist.percentiles();
    let uptime_seconds = ((now_ms().saturating_sub(state.daemon_start)) / 1000) as u64;

    let mut body = state.system_metrics.render_prometheus(PrometheusSample {
        queue_depth,
        active_node_count,
        node_latency_ms,
        scheduler_decision_latency_ms: 0,
        e2e_latency_p50_ms: p.p50_ms,
        e2e_latency_p95_ms: p.p95_ms,
        e2e_latency_p99_ms: p.p99_ms,
        node_uptime_seconds: uptime_seconds,
    });
    let bootstrap_connected = if let Some(ring) = state.bootstrap_ring.as_ref() {
        ring.connected_count().await
    } else {
        0
    };
    body.push_str(
        "# HELP shard_bootstrap_connected_count Number of bootstrap peers currently connected\n",
    );
    body.push_str("# TYPE shard_bootstrap_connected_count gauge\n");
    body.push_str(format!("shard_bootstrap_connected_count {}\n", bootstrap_connected).as_str());

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
    let gossipsub = state.gossipsub_latency_hist.percentiles();
    let now = now_ms();
    prune_stale_speculative_pending(&state, now).await;
    prune_stale_scout_work_queue_for_state(&state, now).await;
    let scout_queue_depth = state.scout_work.lock().await.len();
    let pending_queue_depth = state.speculative_pending.lock().await.len();
    let verifier_in_flight_depth = verifier_in_flight_depth(&state);
    let queue_depth = effective_verifier_queue_depth(&state, pending_queue_depth);
    let (avg_latency_ms, verifier_p95_latency_ms) = verifier_latency_snapshot(&state);
    prune_expired_scout_leases_for_state(&state, now).await;
    let active_leases = state.scout_work_leases.lock().await.len();
    let blackout_mode = {
        let blackout = state.scout_blackout.lock().await;
        scout_blackout_mode(&*blackout, now)
    };
    let mut reports = state.node_metric_reports.lock().await;
    let mut active_nodes = 0usize;
    let mut healthy_nodes = 0usize;
    let mut unhealthy_nodes = 0usize;
    let mut active_scout_nodes = 0usize;

    for snapshot in reports.values_mut() {
        snapshot.healthy =
            node_is_healthy(snapshot.last_report_ms, now, state.heartbeat_timeout_ms);
        if snapshot.role.eq_ignore_ascii_case("scout") {
            active_scout_nodes += 1;
        } else {
            active_nodes += 1;
            if snapshot.healthy {
                healthy_nodes += 1;
            } else {
                unhealthy_nodes += 1;
            }
        }
    }

    let (
        draft_capable_scout_count,
        active_browser_runtime_count,
        scout_runtime_webgpu_total,
        scout_runtime_wasm_total,
        scout_last_submit_success_ms,
    ) = {
        let mut runtime = state.scout_client_runtime.lock().await;
        prune_scout_client_runtime(&mut runtime, now);
        summarize_scout_client_runtime(&runtime, now)
    };
    let scout_submit_success_age_ms = scout_last_submit_success_ms.map(|ts| now.saturating_sub(ts));

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
        "active_scout_nodes".to_string(),
        serde_json::json!(active_scout_nodes),
    );
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
        "scout_ingress_enabled".to_string(),
        serde_json::json!(state.scout_ingress_enabled.load(Ordering::Relaxed)),
    );
    payload.insert(
        "verifier_in_flight_depth".to_string(),
        serde_json::json!(verifier_in_flight_depth),
    );
    payload.insert(
        "speculative_pending_depth".to_string(),
        serde_json::json!(pending_queue_depth),
    );
    payload.insert(
        "scout_work_queue_depth".to_string(),
        serde_json::json!(scout_queue_depth),
    );
    payload.insert(
        "active_leases".to_string(),
        serde_json::json!(active_leases),
    );
    payload.insert(
        "scout_blackout_mode".to_string(),
        serde_json::json!(match blackout_mode {
            ScoutBlackoutMode::Open => "open",
            ScoutBlackoutMode::Blackout => "blackout",
            ScoutBlackoutMode::ReopenStage1 => "reopen_stage_1",
            ScoutBlackoutMode::ReopenStage2 => "reopen_stage_2",
            ScoutBlackoutMode::ReopenStage3 => "reopen_stage_3",
        }),
    );
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
        serde_json::json!(avg_latency_ms),
    );
    payload.insert(
        "p95_latency_ms".to_string(),
        serde_json::json!(verifier_p95_latency_ms),
    );
    payload.insert(
        "p99_latency_ms".to_string(),
        serde_json::json!(gossipsub.p99_ms.max(verifier_p95_latency_ms)),
    );
    payload.insert(
        "gossipsub_p95_latency_ms".to_string(),
        serde_json::json!(gossipsub.p95_ms),
    );
    payload.insert(
        "draft_capable_scouts".to_string(),
        serde_json::json!(draft_capable_scout_count),
    );
    payload.insert(
        "active_browser_sessions".to_string(),
        serde_json::json!(active_browser_runtime_count),
    );
    payload.insert(
        "scout_runtime_webgpu_total".to_string(),
        serde_json::json!(scout_runtime_webgpu_total),
    );
    payload.insert(
        "scout_runtime_wasm_total".to_string(),
        serde_json::json!(scout_runtime_wasm_total),
    );
    payload.insert(
        "scout_last_submit_success_ms".to_string(),
        serde_json::json!(scout_last_submit_success_ms),
    );
    payload.insert(
        "scout_submit_success_age_ms".to_string(),
        serde_json::json!(scout_submit_success_age_ms),
    );
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
        "scout_work_rate_limited_total".to_string(),
        serde_json::json!(counters.scout_work_rate_limited_total),
    );
    payload.insert(
        "scout_work_overload_reject_total".to_string(),
        serde_json::json!(counters.scout_work_overload_reject_total),
    );
    payload.insert(
        "scout_work_active_cap_reject_total".to_string(),
        serde_json::json!(counters.scout_work_active_cap_reject_total),
    );
    payload.insert(
        "scout_work_lease_issued_total".to_string(),
        serde_json::json!(counters.scout_work_lease_issued_total),
    );
    payload.insert(
        "scout_work_lease_expired_total".to_string(),
        serde_json::json!(counters.scout_work_lease_expired_total),
    );
    payload.insert(
        "scout_draft_submissions_total".to_string(),
        serde_json::json!(counters.scout_draft_submissions_total),
    );
    payload.insert(
        "scout_draft_rate_limited_total".to_string(),
        serde_json::json!(counters.scout_draft_rate_limited_total),
    );
    payload.insert(
        "scout_draft_overload_reject_total".to_string(),
        serde_json::json!(counters.scout_draft_overload_reject_total),
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
        "scout_draft_reject_lease_missing_total".to_string(),
        serde_json::json!(counters.scout_draft_reject_lease_missing_total),
    );
    payload.insert(
        "scout_draft_reject_lease_mismatch_total".to_string(),
        serde_json::json!(counters.scout_draft_reject_lease_mismatch_total),
    );
    payload.insert(
        "scout_draft_reject_lease_expired_total".to_string(),
        serde_json::json!(counters.scout_draft_reject_lease_expired_total),
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
        "chat_completion_success_total".to_string(),
        serde_json::json!(counters.chat_completion_success_total),
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
        "output_degeneration_detected_total".to_string(),
        serde_json::json!(counters.output_degeneration_detected_total),
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
        "scout_blackout_enter_total".to_string(),
        serde_json::json!(counters.scout_blackout_enter_total),
    );
    payload.insert(
        "scout_blackout_exit_total".to_string(),
        serde_json::json!(counters.scout_blackout_exit_total),
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
    drop(registry);

    {
        let mut known = state.known_peers.lock().await;
        known.push(req.multiaddr.clone());
        *known = unique_addrs(known.clone());
        save_persisted_peers(state.known_peers_path.as_path(), &known).await;
    }
    let total_registered = state.bootstrap_registry.lock().await.len();

    Json(serde_json::json!({
        "ok": true,
        "message": "Bootstrap registration recorded",
        "total_registered": total_registered,
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        prune_expired_scout_leases, prune_stale_scout_work_queue, push_scheduler_decision_log,
        require_scout_id, runtime_health_state, sanitize_scout_draft_text,
        scout_admission_decision, scout_assignment_backpressured, scout_blackout_mode,
        scout_reopen_stage_ms, scout_work_max_age_ms, should_route_long_context,
        summarize_scout_client_runtime, verify_spot_check_submission, DraftSpotCheckProof,
        ScoutAdmissionMode, ScoutBlackoutMode, ScoutBlackoutState, ScoutClientRuntimeStatus,
        ScoutWorkLease, ScoutWorkQuery, WebGPUProbeResult, WebGPUStats,
    };
    use crate::{SchedulerDecisionLog, WorkRequest};
    use std::collections::{HashMap, VecDeque};

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
            "meta-llama/Llama-3.2-1B",
            "meta-llama/Llama-3.2-1B"
        ));
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
    fn scout_backpressure_disabled_when_queue_and_latency_are_low() {
        let blocked = scout_assignment_backpressured("scout-a", 1, 1200, 1_000_000);
        assert!(!blocked);
    }

    #[test]
    fn scout_backpressure_engages_under_moderate_queue_growth() {
        let blocked_count = (0..20)
            .filter(|offset| {
                scout_assignment_backpressured(
                    "scout-mid",
                    8,
                    2800,
                    1_000_000 + (*offset as u128 * 1_000),
                )
            })
            .count();
        assert!(blocked_count > 0);
    }

    #[test]
    fn scout_backpressure_engages_under_heavy_load() {
        // Probe multiple consecutive windows to avoid hash-bucket flukes.
        let blocked_count = (0..20)
            .filter(|offset| {
                scout_assignment_backpressured(
                    "scout-heavy",
                    900,
                    9000,
                    1_000_000 + (*offset as u128 * 1_000),
                )
            })
            .count();
        assert!(blocked_count > 0);
    }

    #[test]
    fn scout_admission_opens_under_healthy_conditions() {
        let decision = scout_admission_decision(2, 1200, 1800);
        assert_eq!(decision.mode, ScoutAdmissionMode::Allow);
    }

    #[test]
    fn scout_admission_hard_circuit_on_high_p95() {
        let decision = scout_admission_decision(3, 1200, 7000);
        assert_eq!(decision.mode, ScoutAdmissionMode::HardCircuit);
        assert!(decision.retry_after_ms >= 250);
    }

    #[test]
    fn scout_lease_pruning_removes_only_expired_entries() {
        let now = 10_000u128;
        let mut leases = HashMap::new();
        leases.insert(
            "work-expired".to_string(),
            ScoutWorkLease {
                lease_id: "lease-expired".to_string(),
                scout_id: "scout-a".to_string(),
                expires_at_ms: now - 1,
            },
        );
        leases.insert(
            "work-active".to_string(),
            ScoutWorkLease {
                lease_id: "lease-active".to_string(),
                scout_id: "scout-b".to_string(),
                expires_at_ms: now + 5_000,
            },
        );

        let removed = prune_expired_scout_leases(&mut leases, now);
        assert_eq!(removed, 1);
        assert!(leases.contains_key("work-active"));
        assert!(!leases.contains_key("work-expired"));
    }

    #[test]
    fn scout_work_pruning_removes_only_stale_entries() {
        let max_age_ms = scout_work_max_age_ms();
        let now = max_age_ms.saturating_add(10_000);
        let mut queue = std::collections::VecDeque::from([
            WorkRequest {
                request_id: "work-expired".to_string(),
                prompt_context: "hello".to_string(),
                min_tokens: 4,
                created_at_ms: Some(now.saturating_sub(max_age_ms + 1)),
                lease_id: None,
                lease_expires_at_ms: None,
                assigned_scout_id: None,
                preferred_endpoint: None,
            },
            WorkRequest {
                request_id: "work-active".to_string(),
                prompt_context: "hello".to_string(),
                min_tokens: 4,
                created_at_ms: Some(now.saturating_sub(max_age_ms.saturating_sub(1))),
                lease_id: None,
                lease_expires_at_ms: None,
                assigned_scout_id: None,
                preferred_endpoint: None,
            },
        ]);

        let removed = prune_stale_scout_work_queue(&mut queue, now);
        assert_eq!(removed, 1);
        assert_eq!(queue.len(), 1);
        assert_eq!(
            queue.front().map(|work| work.request_id.as_str()),
            Some("work-active")
        );
    }

    #[test]
    fn scout_blackout_mode_transitions_through_reopen_stages() {
        let mut blackout = ScoutBlackoutState::default();
        blackout.blackout_until_ms = 2_000;
        blackout.reopen_started_ms = Some(2_000);

        assert_eq!(
            scout_blackout_mode(&blackout, 1_500),
            ScoutBlackoutMode::Blackout
        );
        assert_eq!(
            scout_blackout_mode(&blackout, 2_000),
            ScoutBlackoutMode::ReopenStage1
        );
        assert!(matches!(
            scout_blackout_mode(&blackout, 2_000 + scout_reopen_stage_ms()),
            ScoutBlackoutMode::ReopenStage2
        ));
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

    #[test]
    fn scout_runtime_summary_counts_active_browser_sessions() {
        let now = 10_000u128;
        let mut statuses = HashMap::new();
        statuses.insert(
            "scout-webgpu".to_string(),
            ScoutClientRuntimeStatus {
                scout_id: "scout-webgpu".to_string(),
                runtime_mode: Some("webgpu".to_string()),
                last_event: "runtime_webgpu_ready".to_string(),
                last_event_detail: None,
                last_event_ms: now - 1_000,
                last_submit_success_ms: Some(now - 500),
            },
        );
        statuses.insert(
            "scout-wasm".to_string(),
            ScoutClientRuntimeStatus {
                scout_id: "scout-wasm".to_string(),
                runtime_mode: Some("wasm".to_string()),
                last_event: "runtime_wasm_fallback".to_string(),
                last_event_detail: None,
                last_event_ms: now - 1_500,
                last_submit_success_ms: None,
            },
        );

        let (
            draft_capable,
            active_browser_sessions,
            webgpu_total,
            wasm_total,
            last_submit_success_ms,
        ) = summarize_scout_client_runtime(&statuses, now);

        assert_eq!(draft_capable, 1);
        assert_eq!(active_browser_sessions, 2);
        assert_eq!(webgpu_total, 1);
        assert_eq!(wasm_total, 1);
        assert_eq!(last_submit_success_ms, Some(now - 500));
    }

    #[test]
    fn webgpu_coverage_summary_counts_and_breakdowns() {
        let mut stats = WebGPUStats::default();
        stats.record_probe(&WebGPUProbeResult {
            eligible: true,
            reason: None,
            tier: "high-performance".to_string(),
            estimated_vram_mb: 8192,
            supports_f16: true,
            browser: "Chrome".to_string(),
            os: "Windows".to_string(),
            adapter_vendor: "NVIDIA".to_string(),
            adapter_device: "RTX".to_string(),
        });
        stats.record_probe(&WebGPUProbeResult {
            eligible: false,
            reason: Some("no_adapter".to_string()),
            tier: "none".to_string(),
            estimated_vram_mb: 0,
            supports_f16: false,
            browser: "Firefox".to_string(),
            os: "Linux".to_string(),
            adapter_vendor: "unknown".to_string(),
            adapter_device: "unknown".to_string(),
        });
        stats.record_probe(&WebGPUProbeResult {
            eligible: false,
            reason: Some("no_navigator_gpu".to_string()),
            tier: "none".to_string(),
            estimated_vram_mb: 0,
            supports_f16: false,
            browser: "Safari".to_string(),
            os: "macOS".to_string(),
            adapter_vendor: "unknown".to_string(),
            adapter_device: "unknown".to_string(),
        });

        let summary = stats.coverage_summary();
        assert_eq!(summary["total_probes"], serde_json::json!(3));
        assert_eq!(summary["eligible_pct"], serde_json::json!(33.3));
        assert_eq!(summary["high_performance_pct"], serde_json::json!(33.3));
        assert_eq!(summary["low_power_pct"], serde_json::json!(0.0));
        assert_eq!(summary["ineligible_pct"], serde_json::json!(66.7));
        assert_eq!(
            summary["ineligible_reason_breakdown"]["no_adapter"],
            serde_json::json!(33.3)
        );
        assert_eq!(
            summary["ineligible_reason_breakdown"]["no_navigator_gpu"],
            serde_json::json!(33.3)
        );
    }
}

pub(crate) async fn leaderboard_handler(
    AxumState(_state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "ok": true,
        "leaderboard": []
    }))
}
