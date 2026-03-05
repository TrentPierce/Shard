use super::*;
use axum::http::{HeaderName, HeaderValue, StatusCode};
use libp2p::multiaddr::Protocol;
use reqwest::Url;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::Ordering;
use std::time::Duration;

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

fn should_refuse_mesh_degraded(
    refuse_work_below_min_bootstrap: bool,
    mesh_healthy: bool,
    local_request: bool,
) -> bool {
    refuse_work_below_min_bootstrap && !mesh_healthy && !local_request
}

fn host_is_local(host: &str) -> bool {
    let raw = host.trim().trim_start_matches('[').trim_end_matches(']');
    let host_only = raw.split(':').next().unwrap_or(raw);
    matches!(host_only, "localhost" | "127.0.0.1" | "::1")
}

fn request_host_is_local(headers: &HeaderMap) -> bool {
    headers
        .get("host")
        .and_then(|value| value.to_str().ok())
        .map(host_is_local)
        .unwrap_or(false)
}

fn infer_client_ip(headers: &HeaderMap) -> Option<String> {
    if let Some(forwarded) = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
    {
        if let Some(ip) = forwarded.split(',').next().map(str::trim) {
            if !ip.is_empty() {
                return Some(ip.to_string());
            }
        }
    }
    headers
        .get("x-real-ip")
        .and_then(|value| value.to_str().ok())
        .or_else(|| {
            headers
                .get("cf-connecting-ip")
                .and_then(|value| value.to_str().ok())
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_bool_flag(raw: Option<&str>) -> Option<bool> {
    raw.map(|value| {
        !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        )
    })
}

fn mesh_forward_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        parse_bool_flag(std::env::var("SHARD_MESH_FORWARD_ENABLED").ok().as_deref()).unwrap_or(true)
    })
}

fn mesh_forward_port() -> u16 {
    static PORT: std::sync::OnceLock<u16> = std::sync::OnceLock::new();
    *PORT.get_or_init(|| {
        std::env::var("SHARD_MESH_FORWARD_PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(9091)
    })
}

fn mesh_forward_max_hops() -> u8 {
    static MAX_HOPS: std::sync::OnceLock<u8> = std::sync::OnceLock::new();
    *MAX_HOPS.get_or_init(|| {
        std::env::var("SHARD_MESH_FORWARD_MAX_HOPS")
            .ok()
            .and_then(|v| v.parse::<u8>().ok())
            .map(|v| v.min(4))
            .unwrap_or(1)
    })
}

fn mesh_forward_probe_limit() -> usize {
    static LIMIT: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *LIMIT.get_or_init(|| {
        std::env::var("SHARD_MESH_FORWARD_PROBE_LIMIT")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .map(|v| v.clamp(1, 12))
            .unwrap_or(4)
    })
}

fn mesh_forward_probe_timeout_ms() -> u64 {
    static TIMEOUT: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *TIMEOUT.get_or_init(|| {
        std::env::var("SHARD_MESH_FORWARD_PROBE_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(|v| v.clamp(100, 5_000))
            .unwrap_or(600)
    })
}

fn mesh_forward_request_timeout_ms() -> u64 {
    static TIMEOUT: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *TIMEOUT.get_or_init(|| {
        std::env::var("SHARD_MESH_FORWARD_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(|v| v.clamp(250, 120_000))
            .unwrap_or(20_000)
    })
}

fn mesh_forward_queue_weight_ms() -> f64 {
    static WEIGHT: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *WEIGHT.get_or_init(|| {
        std::env::var("SHARD_MESH_FORWARD_QUEUE_WEIGHT_MS")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v >= 0.0)
            .unwrap_or(120.0)
    })
}

fn mesh_forward_min_improvement_ms() -> f64 {
    static MIN_IMPROVEMENT: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *MIN_IMPROVEMENT.get_or_init(|| {
        std::env::var("SHARD_MESH_FORWARD_MIN_IMPROVEMENT_MS")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v >= 0.0)
            .unwrap_or(120.0)
    })
}

fn mesh_forward_local_queue_trigger() -> f64 {
    static TRIGGER: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *TRIGGER.get_or_init(|| {
        std::env::var("SHARD_MESH_FORWARD_LOCAL_QUEUE_TRIGGER")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v >= 0.0)
            .unwrap_or(2.0)
    })
}

fn mesh_forward_current_hop(headers: &HeaderMap) -> u8 {
    headers
        .get("x-shard-forward-hop")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(0)
}

fn should_attempt_mesh_forward(headers: &HeaderMap, route_private: bool, stream_mode: bool) -> bool {
    if route_private || stream_mode || !mesh_forward_enabled() {
        return false;
    }
    if matches!(
        parse_bool_flag(
            headers
                .get("x-shard-mesh-forward")
                .and_then(|value| value.to_str().ok())
        ),
        Some(false)
    ) {
        return false;
    }
    mesh_forward_current_hop(headers) < mesh_forward_max_hops()
}

fn normalize_endpoint(raw: &str, default_port: u16) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let with_scheme = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };
    let mut url = Url::parse(&with_scheme).ok()?;
    if url.host_str().is_none() {
        return None;
    }
    if url.port().is_none() {
        url.set_port(Some(default_port)).ok()?;
    }
    let scheme = if url.scheme().eq_ignore_ascii_case("https") {
        "https"
    } else {
        "http"
    };
    let host = url.host_str()?.to_string();
    let host_fmt = if host.contains(':') {
        format!("[{host}]")
    } else {
        host
    };
    let port = url.port_or_known_default().unwrap_or(default_port);
    Some(format!("{scheme}://{host_fmt}:{port}"))
}

fn endpoint_from_multiaddr(addr: &str, control_port: u16) -> Option<String> {
    let parsed = addr.parse::<libp2p::Multiaddr>().ok()?;
    let mut host: Option<String> = None;
    let mut has_relay = false;
    for protocol in parsed.iter() {
        match protocol {
            Protocol::Ip4(ip) => host = Some(ip.to_string()),
            Protocol::Ip6(ip) => host = Some(ip.to_string()),
            Protocol::Dns(name) | Protocol::Dns4(name) | Protocol::Dns6(name) => {
                host = Some(name.to_string())
            }
            Protocol::P2pCircuit => {
                has_relay = true;
            }
            _ => {}
        }
    }
    if has_relay {
        return None;
    }
    normalize_endpoint(host?.as_str(), control_port)
}

fn json_number(value: Option<&serde_json::Value>) -> Option<f64> {
    value.and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|x| x as f64)))
}

fn mesh_forward_score(latency_ms: f64, queue_depth: f64, queue_weight_ms: f64) -> f64 {
    latency_ms.max(0.0) + queue_depth.max(0.0) * queue_weight_ms.max(0.0)
}

fn should_forward_to_mesh(
    local_score: f64,
    best_remote_score: f64,
    local_queue_depth: f64,
    min_improvement_ms: f64,
    local_queue_trigger: f64,
) -> bool {
    if !local_score.is_finite() || !best_remote_score.is_finite() {
        return false;
    }
    local_queue_depth >= local_queue_trigger
        || best_remote_score + min_improvement_ms <= local_score
}

fn mesh_forward_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

#[derive(Debug, Clone)]
struct MeshEndpointCandidate {
    endpoint: String,
    peer_latency_ms: f64,
}

#[derive(Debug, Clone)]
struct MeshEndpointScore {
    endpoint: String,
    queue_depth: f64,
    latency_ms: f64,
    score: f64,
}

async fn discover_mesh_endpoints(state: &SharedState) -> Vec<MeshEndpointCandidate> {
    let control_port = mesh_forward_port();
    let mut discovered: HashMap<String, f64> = HashMap::new();

    if let Ok(raw) = std::env::var("SHARD_MESH_FORWARD_ENDPOINTS") {
        for endpoint in raw.split(',') {
            if let Some(normalized) = normalize_endpoint(endpoint, control_port) {
                discovered.entry(normalized).or_insert(300.0);
            }
        }
    }

    let peers = state.peers.lock().await.clone();
    for peer in peers.values() {
        let peer_latency = if peer.avg_latency_ms > 0.0 {
            peer.avg_latency_ms as f64
        } else {
            300.0
        };
        for addr in &peer.addrs {
            if let Some(endpoint) = endpoint_from_multiaddr(addr, control_port) {
                let entry = discovered.entry(endpoint).or_insert(peer_latency);
                if peer_latency < *entry {
                    *entry = peer_latency;
                }
            }
        }
    }

    let mut out = discovered
        .into_iter()
        .filter_map(|(endpoint, peer_latency_ms)| {
            let host = Url::parse(endpoint.as_str())
                .ok()
                .and_then(|url| url.host_str().map(|h| h.to_string()))
                .unwrap_or_default();
            if host.eq_ignore_ascii_case("localhost")
                || host == "127.0.0.1"
                || host == "::1"
                || host.is_empty()
            {
                return None;
            }
            Some(MeshEndpointCandidate {
                endpoint,
                peer_latency_ms,
            })
        })
        .collect::<Vec<_>>();

    out.sort_by(|a, b| {
        a.peer_latency_ms
            .total_cmp(&b.peer_latency_ms)
            .then_with(|| a.endpoint.cmp(&b.endpoint))
    });
    out
}

async fn score_mesh_endpoint(
    client: &reqwest::Client,
    endpoint: &str,
    queue_weight_ms: f64,
) -> Option<MeshEndpointScore> {
    let timeout = Duration::from_millis(mesh_forward_probe_timeout_ms());
    let response = tokio::time::timeout(
        timeout,
        client.get(format!("{endpoint}/metrics/summary")).send(),
    )
    .await
    .ok()?
    .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let payload = response.json::<serde_json::Value>().await.ok()?;
    let queue_depth = json_number(payload.get("queue_depth"))
        .or_else(|| json_number(payload.get("active_leases")))
        .unwrap_or(0.0);
    let latency_ms = json_number(payload.get("p95_latency_ms"))
        .or_else(|| json_number(payload.get("average_latency_ms")))
        .or_else(|| json_number(payload.get("node_latency_ms")))
        .unwrap_or(0.0);
    Some(MeshEndpointScore {
        endpoint: endpoint.to_string(),
        queue_depth,
        latency_ms,
        score: mesh_forward_score(latency_ms, queue_depth, queue_weight_ms),
    })
}

async fn choose_mesh_forward_target(state: &SharedState) -> Option<MeshEndpointScore> {
    let candidates = discover_mesh_endpoints(state).await;
    if candidates.is_empty() {
        return None;
    }
    let queue_weight_ms = mesh_forward_queue_weight_ms();
    let client = mesh_forward_client();
    let probe_limit = mesh_forward_probe_limit();
    let mut scored = Vec::new();
    for candidate in candidates.into_iter().take(probe_limit) {
        if let Some(score) =
            score_mesh_endpoint(client, candidate.endpoint.as_str(), queue_weight_ms).await
        {
            scored.push(score);
        }
    }
    scored.sort_by(|a, b| {
        a.score
            .total_cmp(&b.score)
            .then_with(|| a.endpoint.cmp(&b.endpoint))
    });
    scored.into_iter().next()
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

fn repetition_detector_min_repeats() -> usize {
    static MIN_REPEATS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *MIN_REPEATS.get_or_init(|| {
        std::env::var("SHARD_OUTPUT_REPETITION_MIN_REPEATS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .map(|v| v.clamp(3, 16))
            .unwrap_or(5)
    })
}

fn repetition_detector_max_unit_chars() -> usize {
    static MAX_UNIT: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *MAX_UNIT.get_or_init(|| {
        std::env::var("SHARD_OUTPUT_REPETITION_MAX_UNIT_CHARS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .map(|v| v.clamp(1, 64))
            .unwrap_or(12)
    })
}

fn detect_repetitive_suffix(text: &str) -> Option<(String, usize)> {
    let min_repeats = repetition_detector_min_repeats();
    let max_unit_chars = repetition_detector_max_unit_chars();
    let chars: Vec<char> = text.chars().collect();
    if chars.len() < min_repeats {
        return None;
    }
    let max_unit = (chars.len() / min_repeats).min(max_unit_chars);
    if max_unit == 0 {
        return None;
    }

    for unit_len in 1..=max_unit {
        let unit_start = chars.len().saturating_sub(unit_len);
        let unit = &chars[unit_start..];
        let mut repeats = 1usize;
        while unit_len.saturating_mul(repeats + 1) <= chars.len() {
            let seg_start = chars.len().saturating_sub(unit_len * (repeats + 1));
            let seg_end = seg_start + unit_len;
            if chars[seg_start..seg_end] == *unit {
                repeats += 1;
            } else {
                break;
            }
        }
        if repeats >= min_repeats {
            let repeated_unit: String = unit.iter().collect();
            if repeated_unit.trim().is_empty() {
                continue;
            }
            return Some((repeated_unit, repeats));
        }
    }
    None
}

fn should_abort_on_degenerate_output(existing: &str, next_piece: &str) -> Option<(String, usize)> {
    let mut candidate = String::with_capacity(existing.len().saturating_add(next_piece.len()));
    candidate.push_str(existing);
    candidate.push_str(next_piece);
    detect_repetitive_suffix(candidate.as_str())
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
            .unwrap_or(18.0)
    })
}

fn speculative_top_k() -> usize {
    static TOP_K: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *TOP_K.get_or_init(|| {
        std::env::var("SHARD_SPECULATIVE_TOP_K")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .map(|v| v.clamp(1, 100))
            .unwrap_or(20)
    })
}

const DEFAULT_SCOUT_WORK_QUEUE_MAX: usize = 1024;

pub(crate) fn scout_work_queue_max() -> usize {
    std::env::var("SHARD_SCOUT_WORK_QUEUE_MAX")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .map(|v| v.clamp(64, 4096))
        .unwrap_or(DEFAULT_SCOUT_WORK_QUEUE_MAX)
}

pub(crate) fn enqueue_scout_work(
    queue: &mut std::collections::VecDeque<WorkRequest>,
    work: WorkRequest,
) {
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
        Err(tokio::sync::mpsc::error::TrySendError::Full(work)) => {
            // Best-effort fallback when the broadcast channel is briefly saturated.
            // This avoids losing speculative opportunities during short bursts.
            let work_tx = state.work_tx.clone();
            tokio::spawn(async move {
                match tokio::time::timeout(std::time::Duration::from_millis(25), work_tx.send(work))
                    .await
                {
                    Ok(Ok(())) => {
                        tracing::debug!("work publish channel recovered via fallback send");
                    }
                    Ok(Err(_)) => {
                        tracing::warn!("work publish channel closed during fallback send");
                    }
                    Err(_) => {
                        tracing::warn!("work publish channel saturated; fallback send timed out");
                    }
                }
            });
        }
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
            tracing::warn!("work publish channel closed; unable to broadcast");
        }
    }
}

fn local_scout_fallback_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("SHARD_LOCAL_SCOUT_FALLBACK")
            .ok()
            .map(|v| {
                let lowered = v.trim().to_ascii_lowercase();
                !matches!(lowered.as_str(), "0" | "false" | "no" | "off")
            })
            .unwrap_or(true)
    })
}

fn local_scout_fallback_delay_ms() -> u64 {
    static DELAY_MS: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *DELAY_MS.get_or_init(|| {
        std::env::var("SHARD_LOCAL_SCOUT_FALLBACK_DELAY_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(|v| v.clamp(0, 10_000))
            .unwrap_or(250)
    })
}

fn local_scout_fallback_remote_delay_ms() -> u64 {
    static DELAY_MS: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *DELAY_MS.get_or_init(|| {
        std::env::var("SHARD_LOCAL_SCOUT_FALLBACK_REMOTE_DELAY_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(|v| v.clamp(50, 15_000))
            .unwrap_or(600)
    })
}

fn local_scout_fallback_remote_inflight_cap() -> usize {
    static CAP: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *CAP.get_or_init(|| {
        std::env::var("SHARD_LOCAL_SCOUT_FALLBACK_REMOTE_INFLIGHT_CAP")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .map(|v| v.clamp(1, 512))
            .unwrap_or(6)
    })
}

fn local_scout_fallback_remote_pending_cap() -> usize {
    static CAP: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *CAP.get_or_init(|| {
        std::env::var("SHARD_LOCAL_SCOUT_FALLBACK_REMOTE_PENDING_CAP")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .map(|v| v.clamp(1, 512))
            .unwrap_or(8)
    })
}

fn local_scout_fallback_remote_latency_cap_ms() -> u64 {
    static CAP: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *CAP.get_or_init(|| {
        std::env::var("SHARD_LOCAL_SCOUT_FALLBACK_REMOTE_LATENCY_CAP_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(|v| v.clamp(250, 60_000))
            .unwrap_or(2_500)
    })
}

fn local_scout_fallback_allowed(
    has_remote_scouts: bool,
    in_flight: usize,
    pending_depth: usize,
    avg_latency_ms: u64,
) -> bool {
    if !has_remote_scouts {
        return true;
    }
    in_flight <= local_scout_fallback_remote_inflight_cap()
        && pending_depth <= local_scout_fallback_remote_pending_cap()
        && avg_latency_ms <= local_scout_fallback_remote_latency_cap_ms()
}

async fn local_daemon_draft_capable(state: &SharedState) -> bool {
    let contribute_enabled = {
        let topo = state.topology.lock().await;
        topo.contribute_enabled
    };
    if !contribute_enabled {
        return false;
    }
    let engine_guard = state.engine.lock().await;
    engine_guard.is_some()
}

async fn generate_local_daemon_draft(
    state: &SharedState,
    work: &WorkRequest,
) -> Option<WorkResponse> {
    if !local_daemon_draft_capable(state).await {
        return None;
    }
    let still_pending = {
        let pending = state.speculative_pending.lock().await;
        pending.contains_key(work.request_id.as_str())
    };
    if !still_pending {
        return None;
    }
    let local_peer_id = {
        let topo = state.topology.lock().await;
        topo.local_peer_id.clone()
    };
    let draft_start = now_ms();
    let mut engine_guard = match state.engine.try_lock() {
        Ok(guard) => guard,
        Err(_) => {
            tracing::debug!(
                request_id = %work.request_id,
                "skipping local scout fallback draft because engine is busy"
            );
            return None;
        }
    };
    let engine = engine_guard.as_mut()?;

    let mut tokens = engine.tokenize(&work.prompt_context, 4096).ok()?;
    if !tokens.is_empty() && tokens[0] == 128000 {
        tokens.remove(0);
    }
    if engine.eval(&tokens).is_err() {
        return None;
    }

    let target = (work.min_tokens.max(4) as usize).min(32);
    let mut draft_tokens = Vec::with_capacity(target);
    let mut draft_text = String::new();

    for _ in 0..target {
        let logits = engine.get_logits(128256).ok()?;
        let mut best_idx = 0usize;
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
        draft_tokens.push(best_idx as i32);
        if let Ok(piece) = engine.token_to_piece(best_idx as i32) {
            draft_text.push_str(&piece);
        }
        if engine.eval(&[best_idx as i32]).is_err() {
            break;
        }
    }

    if draft_tokens.is_empty() {
        return None;
    }

    let latency_ms = (now_ms().saturating_sub(draft_start)) as f32;
    Some(WorkResponse {
        request_id: work.request_id.clone(),
        peer_id: local_peer_id,
        draft_tokens,
        draft_text,
        latency_ms,
        created_at_ms: Some(now_ms()),
    })
}

async fn publish_local_daemon_draft(state: &SharedState, response: WorkResponse) {
    state.system_metrics.inc_scout_draft_submission();

    {
        let mut by_id = state.idempotent_results.lock().await;
        by_id.insert(response.request_id.clone(), response.clone());
    }

    {
        let draft = scout_draft_from_work_response(&response);
        let mut mailbox = state.scout_draft_mailbox.lock().await;
        let queue = mailbox
            .entry(draft.work_id.clone())
            .or_insert_with(std::collections::VecDeque::new);
        queue.push_back(draft);
        while queue.len() > 8 {
            queue.pop_front();
        }
    }

    {
        let notifiers = state.scout_draft_notifiers.lock().await;
        if let Some(notify) = notifiers.get(&response.request_id) {
            notify.notify_waiters();
        }
    }
}

async fn schedule_local_scout_fallback(state: &SharedState, work: &WorkRequest) {
    if !local_scout_fallback_enabled() {
        return;
    }
    if !local_daemon_draft_capable(state).await {
        return;
    }
    // Hybrid fallback: give remote scouts first chance, then trigger local draft
    // generation to cap tail latency.
    let has_remote_scouts = estimate_remote_active_scouts(state).await > 0;
    let in_flight = state.in_flight_count.load(Ordering::Relaxed);
    let pending_depth = state.speculative_pending.lock().await.len();
    let avg_latency_ms = state.avg_latency_ms.load(Ordering::Relaxed) as u64;
    if !local_scout_fallback_allowed(has_remote_scouts, in_flight, pending_depth, avg_latency_ms) {
        tracing::debug!(
            request_id = %work.request_id,
            has_remote_scouts,
            in_flight,
            pending_depth,
            avg_latency_ms,
            "skipping local scout fallback due to verifier load"
        );
        return;
    }

    let delay_ms = if has_remote_scouts {
        local_scout_fallback_remote_delay_ms()
    } else {
        local_scout_fallback_delay_ms()
    };
    let state_clone = state.clone();
    let work_clone = work.clone();
    tokio::spawn(async move {
        if delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }
        let still_pending = {
            let pending = state_clone.speculative_pending.lock().await;
            pending.contains_key(work_clone.request_id.as_str())
        };
        if !still_pending {
            return;
        }
        let in_flight = state_clone.in_flight_count.load(Ordering::Relaxed);
        let pending_depth = state_clone.speculative_pending.lock().await.len();
        let avg_latency_ms = state_clone.avg_latency_ms.load(Ordering::Relaxed) as u64;
        if !local_scout_fallback_allowed(
            has_remote_scouts,
            in_flight,
            pending_depth,
            avg_latency_ms,
        ) {
            return;
        }
        if let Some(response) = generate_local_daemon_draft(&state_clone, &work_clone).await {
            tracing::debug!(
                request_id = %work_clone.request_id,
                draft_tokens = response.draft_tokens.len(),
                "local daemon scout fallback produced draft"
            );
            publish_local_daemon_draft(&state_clone, response).await;
        }
    });
}

fn compute_effective_scout_timeout_ms(
    base_timeout_ms: u64,
    active_scouts: usize,
    queue_depth: usize,
) -> u64 {
    if active_scouts == 0 {
        return 0;
    }
    // Keep speculative waits bounded while allowing enough budget for real scout
    // round-trips (poll + local generation + submit), especially multi-node WAN.
    let bounded_base = base_timeout_ms.clamp(600, 5_000);
    let soft_queue = std::env::var("SHARD_SCOUT_TIMEOUT_QUEUE_SOFT")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .map(|v| v.clamp(1, 256))
        .unwrap_or(6);
    let hard_queue = std::env::var("SHARD_SCOUT_TIMEOUT_QUEUE_HARD")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .map(|v| v.clamp(soft_queue, 512))
        .unwrap_or(12);
    let bypass_queue = std::env::var("SHARD_SCOUT_TIMEOUT_QUEUE_BYPASS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .map(|v| v.clamp(hard_queue, 1024))
        .unwrap_or(20);

    // Strong backpressure: once verifier queueing grows, cut scout wait budget
    // aggressively so speculative routing does not amplify saturation.
    if queue_depth >= bypass_queue {
        return 0;
    }
    if queue_depth >= hard_queue {
        return bounded_base.min(600);
    }
    if queue_depth >= soft_queue {
        return bounded_base.min(800);
    }
    if active_scouts <= 1 {
        return bounded_base.min(1_100);
    }
    if active_scouts <= 3 {
        return bounded_base.min(950);
    }
    bounded_base.min(850)
}

fn update_request_latency_ewma(state: &SharedState, latency_ms: u64) {
    let sample = latency_ms.min(u32::MAX as u64) as u32;
    let previous = state.avg_latency_ms.load(Ordering::Relaxed);
    let next = if previous == 0 {
        sample
    } else {
        // Lightweight EWMA to track verifier request latency for admission control.
        ((((previous as u64) * 7) + ((sample as u64) * 3)) / 10) as u32
    };
    state.avg_latency_ms.store(next, Ordering::Relaxed);
}

#[derive(Debug, Clone, Copy)]
struct ScoutSupplyEstimate {
    productive_runtime_scouts: usize,
    browser_draft_capable: usize,
    recent_pollers: usize,
    recent_submitters: usize,
    recent_result_submitters: usize,
    healthy_scout_reports: usize,
}

impl ScoutSupplyEstimate {
    fn remote_active_scouts(self) -> usize {
        // Route speculative work only when we have proof of recent draft productivity.
        self.productive_runtime_scouts
            .max(self.recent_result_submitters)
    }

    fn candidate_remote_scouts(self) -> usize {
        // Candidate signals are used only for occasional cold-start probes.
        // Accept lightweight scout activity as a hint, but keep "active" gating
        // strict via remote_active_scouts().
        let activity_candidates = if self.recent_submitters > 0 {
            self.recent_submitters
        } else if self.recent_pollers >= 2 {
            1
        } else {
            0
        };
        self.browser_draft_capable
            .max(self.healthy_scout_reports)
            .max(activity_candidates)
    }

    fn effective_active_scouts(self) -> usize {
        self.remote_active_scouts()
    }
}

async fn estimate_scout_supply(state: &SharedState) -> ScoutSupplyEstimate {
    const SCOUT_RUNTIME_TTL_MS: u128 = 10 * 60 * 1000;
    const SCOUT_ACTIVE_WINDOW_MS: u128 = 3 * 60 * 1000;
    const SCOUT_PRODUCTIVE_WINDOW_MS: u128 = 90 * 1000;
    const SCOUT_POLL_ACTIVE_WINDOW_MS: u128 = 60 * 1000;
    const SCOUT_SUBMIT_ACTIVE_WINDOW_MS: u128 = 90 * 1000;
    const SCOUT_RESULT_ACTIVE_WINDOW_MS: u128 = 90 * 1000;
    let now = now_ms();

    let browser_draft_capable = {
        let mut runtime = state.scout_client_runtime.lock().await;
        runtime
            .retain(|_, status| now.saturating_sub(status.last_event_ms) <= SCOUT_RUNTIME_TTL_MS);
        runtime
            .values()
            .filter(|status| {
                status
                    .runtime_mode
                    .as_deref()
                    .map(|mode| mode.eq_ignore_ascii_case("webgpu"))
                    .unwrap_or(false)
                    && now.saturating_sub(status.last_event_ms) <= SCOUT_ACTIVE_WINDOW_MS
            })
            .count()
    };

    let productive_runtime_scouts = {
        let runtime = state.scout_client_runtime.lock().await;
        runtime
            .values()
            .filter(|status| {
                status
                    .runtime_mode
                    .as_deref()
                    .map(|mode| mode.eq_ignore_ascii_case("webgpu"))
                    .unwrap_or(false)
                    && status
                        .last_submit_success_ms
                        .map(|ts| now.saturating_sub(ts) <= SCOUT_PRODUCTIVE_WINDOW_MS)
                        .unwrap_or(false)
            })
            .count()
    };

    let recent_pollers = {
        let mut polls = state.scout_work_last_poll.lock().await;
        polls.retain(|_, ts| now.saturating_sub(*ts) <= SCOUT_RUNTIME_TTL_MS);
        polls
            .values()
            .filter(|ts| now.saturating_sub(**ts) <= SCOUT_POLL_ACTIVE_WINDOW_MS)
            .count()
    };

    let recent_submitters = {
        let mut submits = state.scout_draft_last_submit.lock().await;
        submits.retain(|_, ts| now.saturating_sub(*ts) <= SCOUT_RUNTIME_TTL_MS);
        submits
            .values()
            .filter(|ts| now.saturating_sub(**ts) <= SCOUT_SUBMIT_ACTIVE_WINDOW_MS)
            .count()
    };

    let healthy_scout_reports = {
        let reports = state.node_metric_reports.lock().await;
        reports
            .values()
            .filter(|snapshot| {
                snapshot.role.eq_ignore_ascii_case("scout")
                    && node_is_healthy(snapshot.last_report_ms, now, state.heartbeat_timeout_ms)
            })
            .count()
    };

    let local_peer_id = {
        let topo = state.topology.lock().await;
        topo.local_peer_id.clone()
    };

    let recent_result_submitters = {
        let results = state.results.lock().await;
        let cutoff = now.saturating_sub(SCOUT_RESULT_ACTIVE_WINDOW_MS);
        let mut unique = std::collections::HashSet::new();
        for entry in results.iter() {
            if entry.created_at_ms.unwrap_or(0) >= cutoff
                && !entry.peer_id.trim().is_empty()
                && entry.peer_id != local_peer_id
            {
                unique.insert(entry.peer_id.clone());
            }
        }
        unique.len()
    };

    ScoutSupplyEstimate {
        productive_runtime_scouts,
        browser_draft_capable,
        recent_pollers,
        recent_submitters,
        recent_result_submitters,
        healthy_scout_reports,
    }
}

async fn estimate_remote_active_scouts(state: &SharedState) -> usize {
    estimate_scout_supply(state).await.remote_active_scouts()
}

fn scout_probe_every_n_requests() -> u64 {
    std::env::var("SHARD_SCOUT_PROBE_EVERY_N")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|v| v.clamp(2, 128))
        .unwrap_or(8)
}

fn scout_probe_timeout_ms() -> u64 {
    std::env::var("SHARD_SCOUT_PROBE_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|v| v.clamp(150, 1_000))
        .unwrap_or(500)
}

fn scout_probe_queue_max() -> usize {
    std::env::var("SHARD_SCOUT_PROBE_QUEUE_MAX")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .map(|v| v.clamp(2, 64))
        .unwrap_or(12)
}

fn probe_allowed_for_request(request_id: &str, modulus: u64) -> bool {
    if modulus <= 1 {
        return true;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    request_id.hash(&mut hasher);
    (hasher.finish() % modulus) == 0
}

async fn effective_speculative_timeout_ms(
    state: &SharedState,
    config: &SpeculativeConfig,
    request_id: &str,
) -> u64 {
    let supply = estimate_scout_supply(state).await;
    let active_scouts = supply.effective_active_scouts();
    let queue_depth = {
        let scout_queue_depth = state.scout_work.lock().await.len();
        let pending_depth = state.speculative_pending.lock().await.len();
        let verifier_in_flight = state.in_flight_count.load(Ordering::Relaxed);
        scout_queue_depth.max(pending_depth).max(verifier_in_flight)
    };
    let mut timeout_ms =
        compute_effective_scout_timeout_ms(config.scout_timeout_ms, active_scouts, queue_depth);
    if timeout_ms == 0 {
        // Cold-start discovery: only probe occasionally and only when there are
        // scout candidates, with a very short timeout budget.
        let candidate_scouts = supply.candidate_remote_scouts();
        let probe_modulus = scout_probe_every_n_requests();
        let probe_queue_max = scout_probe_queue_max();
        if candidate_scouts > 0
            && queue_depth <= probe_queue_max
            && probe_allowed_for_request(request_id, probe_modulus)
        {
            let probe_timeout = scout_probe_timeout_ms();
            tracing::debug!(
                probe_timeout,
                candidate_scouts,
                probe_modulus,
                probe_queue_max,
                "speculative probe enabled with no productive scouts yet"
            );
            return probe_timeout;
        }
        tracing::debug!(
            productive_runtime_scouts = supply.productive_runtime_scouts,
            browser_draft_capable = supply.browser_draft_capable,
            recent_pollers = supply.recent_pollers,
            recent_submitters = supply.recent_submitters,
            recent_result_submitters = supply.recent_result_submitters,
            healthy_scout_reports = supply.healthy_scout_reports,
            "speculative dispatch skipped: no productive scout supply"
        );
        return 0;
    }

    // ── Acceptance-rate-aware timeout scaling ──
    // If acceptance rate is very low, reduce timeout aggressively so we don't
    // waste TTFT budget waiting for drafts that will be rejected anyway.
    let acceptance_rate = state.system_metrics.speculative_acceptance_rate();
    let verify_attempts = state
        .system_metrics
        .snapshot()
        .speculative_verify_attempts_total;
    if verify_attempts >= 5 && acceptance_rate < 0.25 {
        // When speculative acceptance is weak, aggressively shrink wait budget.
        let scaled = (timeout_ms as f64 * acceptance_rate.max(0.10)) as u64;
        let capped = scaled.clamp(250, 1_000);
        tracing::debug!(
            original_timeout_ms = timeout_ms,
            acceptance_rate = format!("{:.1}%", acceptance_rate * 100.0),
            capped_timeout_ms = capped,
            "reducing scout timeout due to low acceptance rate"
        );
        timeout_ms = capped;
    }

    timeout_ms
}

async fn fetch_speculative_draft(
    state: &SharedState,
    request_id: &str,
    prompt: &str,
    config: &SpeculativeConfig,
) -> Option<ScoutDraft> {
    let in_cooldown = {
        let tracker = state.scout_timeout_tracker.lock().await;
        tracker.is_in_cooldown()
    };
    if in_cooldown {
        tracing::debug!("skipping speculative dispatch while scout timeout cooldown is active");
        return None;
    }

    let work = WorkRequest {
        request_id: request_id.to_string(),
        prompt_context: prompt.to_string(),
        min_tokens: config.draft_token_count as i32,
        created_at_ms: Some(now_ms()),
        lease_id: None,
        lease_expires_at_ms: None,
        assigned_scout_id: None,
        preferred_endpoint: None,
    };

    let scout_timeout_ms = effective_speculative_timeout_ms(state, config, request_id).await;
    if scout_timeout_ms == 0 {
        tracing::debug!("skipping speculative dispatch with zero effective timeout");
        return None;
    }

    dispatch_scout_work(state, work.clone()).await;
    schedule_local_scout_fallback(state, &work).await;

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

async fn clear_speculative_work_state(state: &SharedState, work_id: &str) {
    {
        let mut pending = state.speculative_pending.lock().await;
        pending.remove(work_id);
    }
    {
        let mut leases = state.scout_work_leases.lock().await;
        leases.remove(work_id);
    }
    {
        let mut mailbox = state.scout_draft_mailbox.lock().await;
        mailbox.remove(work_id);
    }
    {
        let mut by_id = state.idempotent_results.lock().await;
        by_id.remove(work_id);
    }
    clear_draft_notifier(state, work_id).await;
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
        pending.entry(work_id.to_string()).or_insert_with(now_ms);
    }
    let start = now_ms();
    let timeout_deadline = start + timeout_ms as u128;

    loop {
        if now_ms() >= timeout_deadline {
            state.system_metrics.inc_speculative_wait_timeout();
            let age_ms = {
                let pending = state.speculative_pending.lock().await;
                pending
                    .get(work_id)
                    .map(|issued| now_ms().saturating_sub(*issued) as u64)
                    .unwrap_or(timeout_ms)
            };
            tracing::warn!(
                work_id = %work_id,
                timeout_ms,
                wait_age_ms = age_ms,
                "scout draft timeout"
            );
            clear_speculative_work_state(state, work_id).await;
            return None;
        }

        if let Some(draft) = pop_mailbox_draft(state, work_id).await {
            state.system_metrics.inc_speculative_wait_hit();
            clear_speculative_work_state(state, work_id).await;
            return Some(draft);
        }

        if let Some(existing) = {
            let mut by_id = state.idempotent_results.lock().await;
            by_id.remove(work_id)
        } {
            state.system_metrics.inc_speculative_wait_hit();
            let draft = scout_draft_from_work_response(&existing);
            clear_speculative_work_state(state, work_id).await;
            return Some(draft);
        }

        let notifier = get_or_create_draft_notifier(state, work_id).await;
        let remaining_ms = timeout_deadline.saturating_sub(now_ms()) as u64;
        let wait_ms = remaining_ms.clamp(25, 250);
        let _ = tokio::time::timeout(
            tokio::time::Duration::from_millis(wait_ms),
            notifier.notified(),
        )
        .await;
    }
}

/// Verify draft tokens against the verifier model.
/// Returns accepted tokens, text, and optionally a token to resample.
///
/// Acceptance policy (any of these causes acceptance):
/// 1. **Greedy match**: draft token == verifier's argmax token
/// 2. **Top-k overlap**: draft token is within the verifier's top-k predictions
/// 3. **Logit gap**: draft token's logit is within tolerance of the best logit
pub(crate) async fn verify_draft_tokens(
    engine: &mut impl shard_verifier::inference::VerifierModel,
    prompt_tokens: &[i32],
    draft_tokens: &[i32],
) -> DraftVerificationResult {
    // 1. Evaluate the prompt context first to build KV cache
    if engine.eval(prompt_tokens).is_err() {
        tracing::warn!("verify_draft_tokens: prompt eval failed");
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

    // 2. Step through each draft token and verify against model predictions
    let logit_tolerance = speculative_logit_tolerance();
    let top_k = speculative_top_k();
    let total_draft = draft_tokens.len();

    for (idx, &draft_token) in draft_tokens.iter().enumerate() {
        if let Ok(logits) = engine.get_logits(vocab_size) {
            // Build top-k indices sorted by descending logit value.
            // Use a partial sort approach: collect (index, logit) pairs,
            // then sort only enough to find the top-k.
            let mut indexed: Vec<(usize, f32)> =
                logits.iter().enumerate().map(|(i, &v)| (i, v)).collect();
            indexed.sort_unstable_by(|a, b| {
                b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
            });

            let best_idx = indexed[0].0;
            let best_val = indexed[0].1;

            let draft_logit = logits
                .get(draft_token as usize)
                .copied()
                .unwrap_or(-f32::INFINITY);
            let logit_gap = best_val - draft_logit;

            // Check acceptance: greedy match, top-k overlap, or logit gap
            let greedy_match = best_idx == draft_token as usize;
            let in_top_k = indexed
                .iter()
                .take(top_k)
                .any(|(i, _)| *i == draft_token as usize);
            let within_tolerance = logit_gap < logit_tolerance;

            let is_accepted = greedy_match || in_top_k || within_tolerance;

            // Find the draft token's rank in the sorted distribution
            let draft_rank = indexed.iter().position(|(i, _)| *i == draft_token as usize);

            let accept_reason = if greedy_match {
                "greedy_match"
            } else if in_top_k {
                "top_k_overlap"
            } else if within_tolerance {
                "logit_tolerance"
            } else {
                "rejected"
            };

            tracing::debug!(
                idx,
                total_draft,
                draft_token,
                best_token = best_idx as i32,
                draft_logit = %format!("{:.3}", draft_logit),
                best_logit = %format!("{:.3}", best_val),
                logit_gap = %format!("{:.3}", logit_gap),
                tolerance = %format!("{:.1}", logit_tolerance),
                draft_rank = ?draft_rank,
                top_k,
                reason = accept_reason,
                accepted = is_accepted,
                "speculative token verification"
            );

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
                // First rejection — log the top-5 for diagnostics
                let top5_tokens: Vec<i32> =
                    indexed.iter().take(5).map(|(i, _)| *i as i32).collect();
                tracing::info!(
                    idx,
                    draft_token,
                    draft_rank = ?draft_rank,
                    logit_gap = %format!("{:.3}", logit_gap),
                    top5 = ?top5_tokens,
                    "draft token rejected — not in top-{top_k}, logit gap {logit_gap:.1} >= tolerance {logit_tolerance:.1}",
                );
                first_rejection_idx = Some(idx);
                let resample_token = Some(best_idx as i32);
                return DraftVerificationResult {
                    accepted_tokens,
                    accepted_text,
                    first_rejection_idx,
                    resample_token,
                };
            }
        } else {
            tracing::warn!(idx, "get_logits failed during draft verification");
            break;
        }
    }

    tracing::info!(
        accepted = accepted_tokens.len(),
        total = total_draft,
        "all draft tokens accepted"
    );
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
    let local_request = request_host_is_local(&headers);
    if let Some(ring) = state.bootstrap_ring.as_ref() {
        if should_refuse_mesh_degraded(
            ring.refuse_work_below_min_bootstrap,
            ring.is_healthy().await,
            local_request,
        ) {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "mesh_degraded",
                    "message": "Insufficient bootstrap connectivity. Retry shortly.",
                })),
            )
                .into_response();
        }
    }

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
    let contribution_balance = if let Some(subject) = api_key {
        let ledger = state.ledger.lock().await;
        ledger.balance_of(subject)
    } else {
        0
    };

    let rate_limit = if let Some(subject) = api_key {
        state
            .rate_limiter
            .check(Some(subject), None, contribution_balance)
    } else if request_host_is_local(&headers) {
        // Local CLI/benchmark calls should not be throttled by shared anonymous key state.
        state.rate_limiter.check(None, None, contribution_balance)
    } else {
        let client_ip = infer_client_ip(&headers).unwrap_or_else(|| "unknown".to_string());
        state
            .rate_limiter
            .check(None, Some(client_ip.as_str()), contribution_balance)
    };
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
    if should_attempt_mesh_forward(&headers, route_private, stream_mode) {
        let queue_weight_ms = mesh_forward_queue_weight_ms();
        let local_queue_depth = state.in_flight_count.load(Ordering::Relaxed) as f64;
        let local_latency_ms = state.avg_latency_ms.load(Ordering::Relaxed) as f64;
        let local_score = mesh_forward_score(local_latency_ms, local_queue_depth, queue_weight_ms);
        if let Some(target) = choose_mesh_forward_target(&state).await {
            if should_forward_to_mesh(
                local_score,
                target.score,
                local_queue_depth,
                mesh_forward_min_improvement_ms(),
                mesh_forward_local_queue_trigger(),
            ) {
                let next_hop = mesh_forward_current_hop(&headers).saturating_add(1);
                let mut request_builder = mesh_forward_client()
                    .post(format!("{}/v1/chat/completions", target.endpoint))
                    .timeout(Duration::from_millis(mesh_forward_request_timeout_ms()))
                    .json(&req)
                    .header("x-shard-forward-hop", next_hop.to_string())
                    .header("x-shard-forwarded-by", state.node_public_key.clone());
                if let Some(value) = headers.get("authorization") {
                    request_builder = request_builder.header("authorization", value);
                }
                if let Some(value) = headers.get("x-shard-inference-mode") {
                    request_builder = request_builder.header("x-shard-inference-mode", value);
                }
                if let Some(value) = headers.get("x-shard-route") {
                    request_builder = request_builder.header("x-shard-route", value);
                }
                match request_builder.send().await {
                    Ok(resp) => {
                        let status = resp.status();
                        let content_type = resp
                            .headers()
                            .get(reqwest::header::CONTENT_TYPE)
                            .cloned();
                        let body = resp.bytes().await.unwrap_or_default();
                        if status.is_success()
                            || (status.is_client_error() && status != reqwest::StatusCode::TOO_MANY_REQUESTS)
                        {
                            let mut out_headers = HeaderMap::new();
                            if let Some(content_type) = content_type {
                                if let Ok(parsed) =
                                    HeaderValue::from_bytes(content_type.as_bytes())
                                {
                                    out_headers.insert(axum::http::header::CONTENT_TYPE, parsed);
                                }
                            }
                            tracing::info!(
                                target = %target.endpoint,
                                queue_depth = target.queue_depth,
                                target_latency_ms = target.latency_ms,
                                local_queue_depth,
                                local_latency_ms,
                                "forwarded chat request to mesh peer"
                            );
                            return (
                                StatusCode::from_u16(status.as_u16())
                                    .unwrap_or(StatusCode::BAD_GATEWAY),
                                out_headers,
                                body,
                            )
                                .into_response();
                        }
                        tracing::warn!(
                            target = %target.endpoint,
                            status = %status,
                            "mesh forward returned retryable status; falling back local"
                        );
                    }
                    Err(error) => {
                        tracing::warn!(
                            target = %target.endpoint,
                            %error,
                            "mesh forward attempt failed; falling back local"
                        );
                    }
                }
            }
        }
    }

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
        .unwrap_or_else(|| "meta-llama/Llama-3.2-1B".to_string());
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
    let mut use_speculative = inference_mode == InferenceMode::Speculative && model_pair_compatible;

    // ── Adaptive Speculative Bypass ──
    // If historical acceptance rate is too low, skip speculative decoding entirely
    // to avoid the costly TTFT penalty (waiting for scouts that produce rejected drafts).
    if use_speculative {
        let bypass_threshold: f64 = std::env::var("SHARD_SPECULATIVE_BYPASS_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.20); // Default: bypass if < 20% acceptance
        let min_samples: u64 = std::env::var("SHARD_SPECULATIVE_BYPASS_MIN_SAMPLES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10); // Need at least 10 verification attempts before bypassing
        let verify_attempts = state
            .system_metrics
            .snapshot()
            .speculative_verify_attempts_total;
        let acceptance_rate = state.system_metrics.speculative_acceptance_rate();

        if verify_attempts >= min_samples && acceptance_rate < bypass_threshold {
            tracing::info!(
                acceptance_rate = format!("{:.1}%", acceptance_rate * 100.0),
                verify_attempts,
                threshold = format!("{:.1}%", bypass_threshold * 100.0),
                "adaptive bypass: skipping speculative path due to low acceptance rate"
            );
            state.system_metrics.inc_speculative_bypass();
            use_speculative = false;
        }
    }

    if stream_mode {
        let stream = async_stream::stream! {
            let mut request_acceptance: Option<(f64, f64)> = None;
            let mut completion_tokens_generated: u64 = 0;
            let mut degeneration_detected = false;
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
                    let mut emitted_text_for_guard = String::new();
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
                                state
                                    .system_metrics
                                    .inc_tokens_offloaded_to_scouts(accepted_count);

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
                                    let acceptance_ratio = if draft_count == 0 {
                                        0.0
                                    } else {
                                        accepted_count as f64 / draft_count as f64
                                    };
                                    let status = penalties.apply_update(ScoutPenaltyUpdate {
                                        peer_id: draft.scout_id.clone(),
                                        accepted: acceptance_ratio >= 0.5,
                                        probability_bound: 0.0,
                                        latency_ms: Some(draft.latency_ms),
                                        reason: result.first_rejection_idx.map(|idx| {
                                            format!(
                                                "Rejected at token {idx} (acceptance {:.0}%)",
                                                acceptance_ratio * 100.0
                                            )
                                        }),
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
                                        emitted_text_for_guard.push_str(clean.as_str());
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

                                if let Ok(raw_piece) = engine.token_to_piece(best_idx as i32) {
                                    let piece = strip_control_tokens(raw_piece.as_str());
                                    if !piece.is_empty() {
                                        if let Some((repeat_unit, repeat_count)) = should_abort_on_degenerate_output(
                                            emitted_text_for_guard.as_str(),
                                            piece.as_str(),
                                        ) {
                                            degeneration_detected = true;
                                            state.system_metrics.inc_output_degeneration_detected();
                                            state.system_metrics.inc_fallback_invocations();
                                            tracing::warn!(
                                                request_id = %request_id,
                                                repeat_unit = %repeat_unit,
                                                repeat_count,
                                                "output degeneration detected during streaming generation; stopping early"
                                            );
                                            break;
                                        }
                                        let chunk = serde_json::json!({
                                            "id": request_id,
                                            "object": "chat.completion.chunk",
                                            "created": now_ms() / 1000,
                                            "model": selected_verifier_model.as_str(),
                                            "choices": [{"index": 0, "delta": {"content": piece}, "finish_reason": serde_json::Value::Null}],
                                        });
                                        yield Ok::<_, std::convert::Infallible>(Event::default().data(chunk.to_string()));
                                        emitted_text_for_guard.push_str(piece.as_str());
                                    }
                                }

                                if engine.eval(&[best_idx as i32]).is_err() {
                                    break;
                                }
                                completion_tokens_generated =
                                    completion_tokens_generated.saturating_add(1);
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

            if degeneration_detected {
                tracing::info!(
                    request_id = %request_id,
                    completion_tokens_generated,
                    "stream response stopped due to degeneration guard"
                );
            }

            state.system_metrics.inc_chat_completion_success();
            state
                .system_metrics
                .inc_tokens_processed(completion_tokens_generated);

            let latency_ms = (now_ms().saturating_sub(request_started_ms)) as u64;
            update_request_latency_ewma(&state, latency_ms);
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
        let mut degeneration_detected = false;
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
                            let result =
                                verify_draft_tokens(engine, &prompt_tokens, &draft.draft_tokens)
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
                            state
                                .system_metrics
                                .inc_tokens_offloaded_to_scouts(accepted_count);

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
                                let acceptance_ratio = if draft_count == 0 {
                                    0.0
                                } else {
                                    accepted_count as f64 / draft_count as f64
                                };
                                let status = penalties.apply_update(
                                    ScoutPenaltyUpdate {
                                        peer_id: draft.scout_id.clone(),
                                        accepted: acceptance_ratio >= 0.5,
                                        probability_bound: 0.0,
                                        latency_ms: Some(draft.latency_ms),
                                        reason: result.first_rejection_idx.map(|idx| {
                                            format!(
                                                "Rejected at token {idx} (acceptance {:.0}%)",
                                                acceptance_ratio * 100.0
                                            )
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
                                let clean = strip_control_tokens(result.accepted_text.as_str());
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

                                if let Ok(piece) = engine.token_to_piece(best_idx as i32) {
                                    let clean = strip_control_tokens(piece.as_str());
                                    if !clean.is_empty() {
                                        if let Some((repeat_unit, repeat_count)) =
                                            should_abort_on_degenerate_output(
                                                full_text.as_str(),
                                                clean.as_str(),
                                            )
                                        {
                                            degeneration_detected = true;
                                            state.system_metrics.inc_output_degeneration_detected();
                                            state.system_metrics.inc_fallback_invocations();
                                            tracing::warn!(
                                                request_id = %request_id,
                                                repeat_unit = %repeat_unit,
                                                repeat_count,
                                                "output degeneration detected during non-stream generation; stopping early"
                                            );
                                            break;
                                        }
                                        full_text.push_str(clean.as_str());
                                    }
                                }

                                if engine.eval(&[best_idx as i32]).is_err() {
                                    break;
                                }
                                completion_tokens_generated =
                                    completion_tokens_generated.saturating_add(1);
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
        update_request_latency_ewma(&state, latency_ms);
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
        state.system_metrics.inc_chat_completion_success();

        if degeneration_detected {
            tracing::info!(
                request_id = %request_id,
                completion_tokens_generated,
                "non-stream response stopped due to degeneration guard"
            );
        }

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
        endpoint_from_multiaddr,
        infer_client_ip, model_pair_acceptance_rates, request_host_is_local, resolve_inference_mode,
        should_attempt_mesh_forward, should_forward_to_mesh, normalize_endpoint, mesh_forward_score,
        local_scout_fallback_allowed, probe_allowed_for_request,
        should_abort_on_degenerate_output, should_refuse_mesh_degraded, strip_control_tokens,
        InferenceMode,
        ScoutSupplyEstimate, WorkRequest,
    };
    use axum::http::{HeaderMap, HeaderValue};
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
                    lease_id: None,
                    lease_expires_at_ms: None,
                    assigned_scout_id: None,
                    preferred_endpoint: None,
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
        assert_eq!(compute_effective_scout_timeout_ms(30_000, 1, 0), 1_100);
        assert_eq!(compute_effective_scout_timeout_ms(30_000, 2, 0), 950);
        assert_eq!(compute_effective_scout_timeout_ms(30_000, 8, 3), 850);
        assert_eq!(compute_effective_scout_timeout_ms(30_000, 8, 8), 800);
        assert_eq!(compute_effective_scout_timeout_ms(30_000, 8, 14), 600);
        assert_eq!(compute_effective_scout_timeout_ms(30_000, 8, 24), 0);
    }

    #[test]
    fn remote_local_fallback_is_blocked_under_load() {
        assert!(local_scout_fallback_allowed(false, 100, 100, 10_000));
        assert!(local_scout_fallback_allowed(true, 2, 3, 800));
        assert!(!local_scout_fallback_allowed(true, 9, 3, 800));
        assert!(!local_scout_fallback_allowed(true, 2, 12, 800));
        assert!(!local_scout_fallback_allowed(true, 2, 3, 3_200));
    }

    #[test]
    fn scout_supply_counts_only_remote_activity() {
        let empty = ScoutSupplyEstimate {
            productive_runtime_scouts: 0,
            browser_draft_capable: 0,
            recent_pollers: 0,
            recent_submitters: 0,
            recent_result_submitters: 0,
            healthy_scout_reports: 0,
        };
        assert_eq!(empty.remote_active_scouts(), 0);
        assert_eq!(empty.effective_active_scouts(), 0);

        let remote_present = ScoutSupplyEstimate {
            productive_runtime_scouts: 1,
            browser_draft_capable: 2,
            recent_pollers: 3,
            recent_submitters: 1,
            recent_result_submitters: 4,
            healthy_scout_reports: 2,
        };
        assert_eq!(remote_present.remote_active_scouts(), 4);
        assert_eq!(remote_present.candidate_remote_scouts(), 2);
        assert_eq!(remote_present.effective_active_scouts(), 4);
    }

    #[test]
    fn probe_sampling_is_deterministic_and_bounded() {
        let request_id = "req-1234";
        let a = probe_allowed_for_request(request_id, 16);
        let b = probe_allowed_for_request(request_id, 16);
        assert_eq!(a, b);
        assert!(probe_allowed_for_request(request_id, 1));
    }

    #[test]
    fn degeneration_guard_detects_repetitive_suffix() {
        assert!(should_abort_on_degenerate_output("endendendend", "end").is_some());
        assert!(should_abort_on_degenerate_output("hello world ", "there").is_none());
    }

    #[test]
    fn host_local_detection_is_correct() {
        let mut local = HeaderMap::new();
        local.insert("host", HeaderValue::from_static("127.0.0.1:9191"));
        assert!(request_host_is_local(&local));

        let mut remote = HeaderMap::new();
        remote.insert("host", HeaderValue::from_static("api.shardnetwork.live"));
        assert!(!request_host_is_local(&remote));
    }

    #[test]
    fn mesh_degraded_refusal_skips_local_requests() {
        assert!(!should_refuse_mesh_degraded(true, false, true));
        assert!(should_refuse_mesh_degraded(true, false, false));
        assert!(!should_refuse_mesh_degraded(true, true, false));
        assert!(!should_refuse_mesh_degraded(false, false, false));
    }

    #[test]
    fn infers_client_ip_from_forwarded_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.8, 10.0.0.1"),
        );
        assert_eq!(infer_client_ip(&headers).as_deref(), Some("203.0.113.8"));

        headers.clear();
        headers.insert("cf-connecting-ip", HeaderValue::from_static("198.51.100.2"));
        assert_eq!(infer_client_ip(&headers).as_deref(), Some("198.51.100.2"));
    }

    #[test]
    fn mesh_forward_respects_headers_and_mode() {
        let headers = HeaderMap::new();
        assert!(!should_attempt_mesh_forward(&headers, false, true));
        assert!(!should_attempt_mesh_forward(&headers, true, false));

        let mut disabled = HeaderMap::new();
        disabled.insert("x-shard-mesh-forward", HeaderValue::from_static("false"));
        assert!(!should_attempt_mesh_forward(&disabled, false, false));
    }

    #[test]
    fn mesh_forward_decision_prefers_lower_score_or_high_local_queue() {
        let local = mesh_forward_score(220.0, 4.0, 120.0);
        let remote = mesh_forward_score(140.0, 1.0, 120.0);
        assert!(should_forward_to_mesh(local, remote, 4.0, 100.0, 2.0));

        let local_low_queue = mesh_forward_score(120.0, 0.0, 120.0);
        let remote_worse = mesh_forward_score(260.0, 1.0, 120.0);
        assert!(!should_forward_to_mesh(
            local_low_queue,
            remote_worse,
            0.0,
            100.0,
            2.0
        ));
    }

    #[test]
    fn endpoint_normalization_handles_scheme_and_port_defaults() {
        assert_eq!(
            normalize_endpoint("api.shardnetwork.live", 9091).as_deref(),
            Some("http://api.shardnetwork.live:9091")
        );
        assert_eq!(
            normalize_endpoint("https://api.shardnetwork.live", 9091).as_deref(),
            Some("https://api.shardnetwork.live:9091")
        );
    }

    #[test]
    fn endpoint_derivation_skips_relay_and_uses_public_host() {
        let peer = "12D3KooWPQqkkZk7NeWA2b1FeWYuBFRW8X7Q9ugymnzxeKJHFLUV";
        let direct = format!("/ip4/35.175.242.222/tcp/4001/p2p/{peer}");
        let relay = format!(
            "/ip4/35.175.242.222/tcp/4001/p2p/{peer}/p2p-circuit/p2p/{peer}"
        );
        assert_eq!(
            endpoint_from_multiaddr(direct.as_str(), 9091).as_deref(),
            Some("http://35.175.242.222:9091")
        );
        assert!(endpoint_from_multiaddr(relay.as_str(), 9091).is_none());
    }
}
