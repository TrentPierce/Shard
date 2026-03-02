use anyhow::Result;
use eframe::egui;
use std::sync::Arc;
use tokio::sync::Mutex;
use tray_icon::menu::MenuEvent;

mod app;
mod process;
mod tray;

use app::ShardApp;
use process::ProcessManager;
use tray::TrayManager;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let _tray_manager = TrayManager::new()?;
    let process_manager = Arc::new(Mutex::new(ProcessManager::new()));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([500.0, 400.0])
            .with_title("Shard Node")
            .with_icon(Arc::new(load_icon())), // Use same icon as tray
        ..Default::default()
    };

    let (telemetry_tx, telemetry_rx) = tokio::sync::mpsc::channel(100);

    eframe::run_native(
        "Shard Node",
        options,
        Box::new(|cc| {
            let pm_for_app = process_manager.clone();
            let app = ShardApp::new(cc, pm_for_app, telemetry_rx);

            // Spawn background task for telemetry and events
            let tx = telemetry_tx.clone();
            tokio::spawn(async move {
                #[derive(serde::Deserialize)]
                struct HealthResp {
                    rust_uptime_ms: Option<u128>,
                    connected_peers: Option<usize>,
                    bitnet_model: Option<String>,
                }

                #[derive(serde::Deserialize)]
                struct TopologyResp {
                    nat_status: Option<String>,
                    relay_mode: Option<bool>,
                    relay_reservation_active: Option<bool>,
                    contribute: Option<bool>,
                }

                #[derive(serde::Deserialize)]
                struct MetricsSummary {
                    speculative_reject_rate: Option<f32>,
                    tokens_processed_total: Option<u64>,
                    tokens_offloaded_to_scouts_total: Option<u64>,
                }

                let client = reqwest::Client::new();

                loop {
                    let health = match client.get("http://127.0.0.1:9091/health").send().await {
                        Ok(r) => r.json::<HealthResp>().await.ok(),
                        Err(_) => None,
                    };
                    let topo = match client.get("http://127.0.0.1:9091/v1/system/topology").send().await {
                        Ok(r) => r.json::<TopologyResp>().await.ok(),
                        Err(_) => None,
                    };
                    let metrics = match client.get("http://127.0.0.1:9091/metrics/summary").send().await {
                        Ok(r) => r.json::<MetricsSummary>().await.ok(),
                        Err(_) => None,
                    };

                    let peers = health.as_ref().and_then(|h| h.connected_peers).unwrap_or(0);
                    let uptime_ms = health.as_ref().and_then(|h| h.rust_uptime_ms).unwrap_or(0);
                    let uptime_secs = uptime_ms / 1000;
                    let uptime = format!(
                        "{:02}:{:02}:{:02}",
                        uptime_secs / 3600,
                        (uptime_secs / 60) % 60,
                        uptime_secs % 60
                    );
                    let role = if topo.as_ref().and_then(|t| t.contribute).unwrap_or(false) {
                        "Shard"
                    } else {
                        "Scout"
                    };
                    let nat_status = topo
                        .as_ref()
                        .and_then(|t| t.nat_status.clone())
                        .unwrap_or_else(|| "unknown".to_string());
                    let relay_status = match (
                        topo.as_ref().and_then(|t| t.relay_mode),
                        topo.as_ref().and_then(|t| t.relay_reservation_active),
                    ) {
                        (Some(true), _) => "server".to_string(),
                        (Some(false), Some(true)) => "reserved".to_string(),
                        (Some(false), Some(false)) => "inactive".to_string(),
                        _ => "unknown".to_string(),
                    };
                    let reject_rate = metrics
                        .as_ref()
                        .and_then(|m| m.speculative_reject_rate)
                        .unwrap_or(0.0);

                    let mut vram_alloc_gb = 0.0;
                    if let Some(path) = health.as_ref().and_then(|h| h.bitnet_model.clone()) {
                        if let Ok(metadata) = std::fs::metadata(path) {
                            vram_alloc_gb = metadata.len() as f32 / (1024.0 * 1024.0 * 1024.0);
                        }
                    }

                    let total_tokens = metrics
                        .as_ref()
                        .and_then(|m| m.tokens_processed_total)
                        .unwrap_or(0)
                        + metrics
                            .as_ref()
                            .and_then(|m| m.tokens_offloaded_to_scouts_total)
                            .unwrap_or(0);

                    let _ = tx
                        .send(app::TelemetryUpdate {
                            role: role.to_string(),
                            peers,
                            tokens: total_tokens,
                            uptime,
                            vram_alloc_gb,
                            vram_limit_gb: 0.0,
                            reject_rate,
                            nat_status,
                            relay_status,
                        })
                        .await;

                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                }
            });

            tokio::spawn(async move {
                let tray_receiver = MenuEvent::receiver();
                loop {
                    while let Ok(event) = tray_receiver.try_recv() {
                        tracing::info!("Tray event: {:?}", event);
                        // If we had the IDs, we'd check them here.
                        // For now, let's just assume any event is a debug signal
                        // and we can add a simple string match if muda provided it.
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            });

            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Eframe error: {}", e))?;

    // On exit, ensure child process is killed
    let mut pm = process_manager.lock().await;
    pm.stop()?;

    Ok(())
}

fn load_icon() -> egui::IconData {
    let (icon_rgba, icon_width, icon_height) = (vec![255_u8; 32 * 32 * 4], 32, 32);
    egui::IconData {
        rgba: icon_rgba,
        width: icon_width,
        height: icon_height,
    }
}
