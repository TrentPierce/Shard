// On Windows: suppress the console window entirely; output goes to the in-app log panel.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use anyhow::Result;
use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use tray_icon::menu::MenuEvent;

mod app;
mod autostart;
mod log_capture;
mod model_download;
mod process;
mod tray;

use app::ShardApp;
use process::DaemonTask;
use tray::TrayManager;

#[tokio::main]
async fn main() -> Result<()> {
    // ── Capture all tracing output into the in-app log panel ────────────────
    // This is installed before the daemon thread starts; the daemon's own
    // try_init() calls will silently no-op since a subscriber is already set.
    let log_buffer = log_capture::new_log_buffer();
    tracing_subscriber::fmt()
        .with_writer(log_capture::LogMakeWriter(log_buffer.clone()))
        .with_ansi(false)
        .without_time()
        .with_level(true)
        .compact()
        .init();

    // ── Tray setup ───────────────────────────────────────────────────────────
    let tray_manager = TrayManager::new()?;
    let show_id = tray_manager.show_id.clone();
    let quit_id = tray_manager.quit_id.clone();
    let pause_id = tray_manager.pause_id.clone();

    let daemon_task = Arc::new(Mutex::new(DaemonTask::new()));

    let show_signal = Arc::new(AtomicBool::new(false));
    let quit_signal = Arc::new(AtomicBool::new(false));
    let pause_signal = Arc::new(AtomicBool::new(false));

    // ── Auto-download model if not configured ────────────────────────────────
    let config = app::AppConfig::load();
    let needs_download = config.bitnet_model_path.is_empty()
        || !std::path::Path::new(&config.bitnet_model_path).exists();

    let (dl_tx, dl_rx) = tokio::sync::mpsc::channel::<model_download::DownloadMsg>(32);
    if needs_download {
        let tx = dl_tx;
        tokio::spawn(model_download::run_download(tx));
    } else {
        drop(dl_tx);
    }

    // ── Telemetry polling ────────────────────────────────────────────────────
    let (telemetry_tx, telemetry_rx) = tokio::sync::mpsc::channel(100);
    {
        let tx = telemetry_tx.clone();
        tokio::spawn(async move {
            #[derive(serde::Deserialize)]
            struct HealthResp {
                rust_uptime_ms: Option<u128>,
                connected_peers: Option<usize>,
                active_scouts: Option<usize>,
                wallet: Option<String>,
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
            #[derive(serde::Deserialize)]
            struct LedgerStats {
                receipt_count: Option<usize>,
            }
            #[derive(serde::Deserialize)]
            struct CreditsResp {
                balance: Option<i64>,
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
                let ledger = match client.get("http://127.0.0.1:9091/ledger/stats").send().await {
                    Ok(r) => r.json::<LedgerStats>().await.ok(),
                    Err(_) => None,
                };
                let wallet = health.as_ref().and_then(|h| h.wallet.clone()).unwrap_or_default();
                let credits = if !wallet.is_empty() {
                    match client.get(format!("http://127.0.0.1:9091/credits/{wallet}")).send().await {
                        Ok(r) => r.json::<CreditsResp>().await.ok().and_then(|c| c.balance).unwrap_or(0),
                        Err(_) => 0,
                    }
                } else {
                    0
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
                let total_tokens = metrics.as_ref().and_then(|m| m.tokens_processed_total).unwrap_or(0)
                    + metrics.as_ref().and_then(|m| m.tokens_offloaded_to_scouts_total).unwrap_or(0);
                let receipts = ledger.and_then(|l| l.receipt_count).unwrap_or(0);

                let _ = tx.send(app::TelemetryUpdate {
                    role: role.to_string(),
                    peers,
                    active_scouts,
                    tokens: total_tokens,
                    uptime,
                    reject_rate,
                    nat_status,
                    relay_status,
                    daemon_online,
                    credits,
                    receipts,
                    wallet,
                }).await;

                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
        });
    }

    // ── Tray event loop ──────────────────────────────────────────────────────
    {
        let show_sig = show_signal.clone();
        let quit_sig = quit_signal.clone();
        let pause_sig = pause_signal.clone();
        tokio::spawn(async move {
            let tray_receiver = MenuEvent::receiver();
            loop {
                while let Ok(event) = tray_receiver.try_recv() {
                    if event.id == show_id { show_sig.store(true, Relaxed); }
                    else if event.id == quit_id { quit_sig.store(true, Relaxed); }
                    else if event.id == pause_id { pause_sig.store(true, Relaxed); }
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        });
    }

    // ── GUI ──────────────────────────────────────────────────────────────────
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([720.0, 560.0])
            .with_min_inner_size([580.0, 440.0])
            .with_title("Shard Node")
            .with_icon(Arc::new(make_icon())),
        ..Default::default()
    };

    eframe::run_native(
        "Shard Node",
        options,
        Box::new(|cc| {
            let app = ShardApp::new(
                cc,
                daemon_task.clone(),
                telemetry_rx,
                dl_rx,
                log_buffer,
                tray_manager,
                show_signal.clone(),
                quit_signal.clone(),
                pause_signal.clone(),
            );
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Eframe error: {}", e))?;

    daemon_task.lock().unwrap().stop();
    Ok(())
}

fn make_icon() -> egui::IconData {
    load_icon_data()
}

/// Decode the bundled PNG logo into raw RGBA bytes at runtime.
fn load_icon_data() -> egui::IconData {
    const PNG: &[u8] = include_bytes!("../assets/icon.png");
    let img = image::load_from_memory(PNG)
        .expect("bundled icon.png is valid")
        .to_rgba8();
    let (width, height) = img.dimensions();
    egui::IconData { rgba: img.into_raw(), width, height }
}

/// Return the same image decoded as a tray-icon Icon.
pub fn make_tray_icon() -> tray_icon::Icon {
    const PNG: &[u8] = include_bytes!("../assets/icon.png");
    let img = image::load_from_memory(PNG)
        .expect("bundled icon.png is valid")
        .to_rgba8();
    let (w, h) = img.dimensions();
    tray_icon::Icon::from_rgba(img.into_raw(), w, h).expect("valid icon")
}
