use crate::{now_ms, SharedState};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use serde::Serialize;
use std::{net::SocketAddr, time::Duration};
use tokio::time;

#[derive(Debug, Serialize)]
struct TelemetrySnapshot {
    connected_peers: usize,
    active_scouts: usize,
    global_tflops: f32,
    sampled_at_ms: u128,
}

pub fn spawn_telemetry_ws_server(state: SharedState, port: u16) {
    tokio::spawn(async move {
        let app = Router::new()
            .route("/telemetry/ws", get(telemetry_ws_handler))
            .with_state(state);

        let addr = SocketAddr::from(([0, 0, 0, 0], port));
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
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| telemetry_stream(socket, state))
}

async fn telemetry_stream(socket: WebSocket, state: SharedState) {
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
