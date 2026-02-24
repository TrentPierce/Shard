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

    let tray_manager = TrayManager::new()?;
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
                let mut count = 0;
                loop {
                    let _ = tx.send(app::TelemetryUpdate {
                        role: "Scout".to_string(),
                        peers: count % 10,
                        tokens: (count * 100) as u64,
                        uptime: format!("{:02}:{:02}:{:02}", count / 3600, (count / 60) % 60, count % 60),
                    }).await;
                    count += 1;
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            });

            let process_mgr_clone = process_manager.clone();
            
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
