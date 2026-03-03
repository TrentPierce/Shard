use anyhow::Result;
use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
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

    let tray_manager = TrayManager::new()?;
    let show_id = tray_manager.show_id.clone();
    let quit_id = tray_manager.quit_id.clone();
    let pause_id = tray_manager.pause_id.clone();

    let process_manager = Arc::new(Mutex::new(ProcessManager::new()));

    let show_signal = Arc::new(AtomicBool::new(false));
    let quit_signal = Arc::new(AtomicBool::new(false));
    let pause_signal = Arc::new(AtomicBool::new(false));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([500.0, 420.0])
            .with_title("Shard Node")
            .with_icon(Arc::new(make_icon())),
        ..Default::default()
    };

    let (telemetry_tx, telemetry_rx) = tokio::sync::mpsc::channel(100);

    // Tray event task: match events to signals
    {
        let show_sig = show_signal.clone();
        let quit_sig = quit_signal.clone();
        let pause_sig = pause_signal.clone();
        tokio::spawn(async move {
            let tray_receiver = MenuEvent::receiver();
            loop {
                while let Ok(event) = tray_receiver.try_recv() {
                    if event.id == show_id {
                        show_sig.store(true, Relaxed);
                    } else if event.id == quit_id {
                        quit_sig.store(true, Relaxed);
                    } else if event.id == pause_id {
                        pause_sig.store(true, Relaxed);
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        });
    }

    // Telemetry polling task
    {
        let tx = telemetry_tx.clone();
        tokio::spawn(async move {
            #[derive(serde::Deserialize)]
            struct HealthResp {
                rust_uptime_ms: Option<u128>,
                connected_peers: Option<usize>,
                active_scouts: Option<usize>,
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
                let topo = match client
                    .get("http://127.0.0.1:9091/v1/system/topology")
                    .send()
                    .await
                {
                    Ok(r) => r.json::<TopologyResp>().await.ok(),
                    Err(_) => None,
                };
                let metrics = match client
                    .get("http://127.0.0.1:9091/metrics/summary")
                    .send()
                    .await
                {
                    Ok(r) => r.json::<MetricsSummary>().await.ok(),
                    Err(_) => None,
                };

                let daemon_online = health.is_some();
                let peers = health.as_ref().and_then(|h| h.connected_peers).unwrap_or(0);
                let active_scouts = health.as_ref().and_then(|h| h.active_scouts).unwrap_or(0);
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
                        active_scouts,
                        tokens: total_tokens,
                        uptime,
                        reject_rate,
                        nat_status,
                        relay_status,
                        daemon_online,
                    })
                    .await;

                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
        });
    }

    eframe::run_native(
        "Shard Node",
        options,
        Box::new(|cc| {
            let pm_for_app = process_manager.clone();
            let app = ShardApp::new(
                cc,
                pm_for_app,
                telemetry_rx,
                tray_manager,
                show_signal.clone(),
                quit_signal.clone(),
                pause_signal.clone(),
            );
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Eframe error: {}", e))?;

    // On exit, ensure child process is killed
    process_manager.lock().unwrap().stop()?;

    Ok(())
}

fn make_icon() -> egui::IconData {
    let width: u32 = 32;
    let height: u32 = 32;
    let cx = width as f32 / 2.0;
    let cy = height as f32 / 2.0;
    let radius = (width as f32 / 2.0) - 1.0;

    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - cx + 0.5;
            let dy = y as f32 - cy + 0.5;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = ((y * width + x) * 4) as usize;
            if dist <= radius {
                rgba[idx] = 0x0E; // R
                rgba[idx + 1] = 0xA5; // G
                rgba[idx + 2] = 0xE9; // B
                rgba[idx + 3] = 255; // A
            }
            // else remains 0,0,0,0 (transparent)
        }
    }

    egui::IconData {
        rgba,
        width,
        height,
    }
}
