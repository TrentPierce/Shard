use crate::{now_ms, SharedState};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use serde::Serialize;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::time;

#[derive(Clone)]
struct TelemetryWsState {
    shared: SharedState,
    token: Option<String>,
    max_connections: Arc<tokio::sync::Semaphore>,
}

#[derive(Debug, Serialize)]
struct TelemetrySnapshot {
    connected_peers: usize,
    active_scouts: usize,
    global_tflops: f32,
    sampled_at_ms: u128,
}

pub(crate) fn spawn_telemetry_ws_server(state: SharedState, port: u16, public_api: bool) {
    tokio::spawn(async move {
        let token = std::env::var("SHARD_TELEMETRY_WS_TOKEN")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        let max_connections = std::env::var("SHARD_TELEMETRY_WS_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .map(|v| v.clamp(1, 10_000))
            .unwrap_or(64);
        let ws_state = TelemetryWsState {
            shared: state,
            token,
            max_connections: Arc::new(tokio::sync::Semaphore::new(max_connections)),
        };

        let app = Router::new()
            .route("/telemetry/ws", get(telemetry_ws_handler))
            .with_state(ws_state);

        let bind_ip = if public_api {
            [0, 0, 0, 0]
        } else {
            [127, 0, 0, 1]
        };
        let addr = SocketAddr::from((bind_ip, port));
        tracing::info!(%addr, "telemetry websocket server starting");

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .expect("failed to bind telemetry websocket port");

        axum::serve(listener, app)
            .await
            .expect("telemetry websocket server crashed");
    });
}

async fn telemetry_ws_handler(
    headers: HeaderMap,
    ws: WebSocketUpgrade,
    State(state): State<TelemetryWsState>,
) -> impl IntoResponse {
    if let Some(required) = &state.token {
        let provided = headers
            .get("x-telemetry-token")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if provided != required {
            return (StatusCode::UNAUTHORIZED, "missing/invalid telemetry token").into_response();
        }
    }

    let permit = match state.max_connections.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                "telemetry connection limit reached",
            )
                .into_response()
        }
    };

    ws.on_upgrade(move |socket| telemetry_stream(socket, state.shared, permit))
}

async fn telemetry_stream(
    socket: WebSocket,
    state: SharedState,
    _permit: tokio::sync::OwnedSemaphorePermit,
) {
    let (mut sender, mut receiver) = socket.split();
    let mut ticker = time::interval(Duration::from_secs(2));

    loop {
        tokio::select! {
            Some(message) = receiver.next() => {
                match message {
                    Ok(Message::Close(_)) => break,
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
            _ = ticker.tick() => {
                let snapshot = collect_snapshot(&state).await;
                let payload = match serde_json::to_string(&snapshot) {
                    Ok(payload) => payload,
                    Err(error) => {
                        tracing::warn!(%error, "failed to serialize telemetry snapshot");
                        continue;
                    }
                };

                if sender.send(Message::Text(payload)).await.is_err() {
                    break;
                }
            }
        }
    }
}

async fn collect_snapshot(state: &SharedState) -> TelemetrySnapshot {
    let peers = state.peers.lock().await;
    let connected_peers = peers.len();
    let active_scouts = peers.values().filter(|peer| peer.verified).count();

    let capacity = state
        .capacity
        .load(std::sync::atomic::Ordering::Relaxed)
        .max(1) as f32;
    let load = state
        .current_load
        .load(std::sync::atomic::Ordering::Relaxed) as f32;
    let utilization = (load / capacity).clamp(0.0, 1.5);
    let estimated_tflops =
        ((capacity * connected_peers.max(1) as f32) / 120.0) * (1.0 + utilization * 0.2);

    TelemetrySnapshot {
        connected_peers,
        active_scouts,
        global_tflops: (estimated_tflops * 100.0).round() / 100.0,
        sampled_at_ms: now_ms(),
    }
}
