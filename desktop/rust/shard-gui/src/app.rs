use anyhow::Result;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex};

use crate::process::{find_daemon_binary, DAEMON_ARGS};
use crate::tray::TrayManager;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    pub bitnet_lib_path: String,
    pub bitnet_model_path: String,
    pub max_ram_gb: f32,
    pub max_vram_gb: f32,
    pub listen_port: u16,
    pub auto_start: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            bitnet_lib_path: String::new(),
            bitnet_model_path: String::new(),
            max_ram_gb: 8.0,
            max_vram_gb: 4.0,
            listen_port: 4001,
            auto_start: true,
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        if let Some(proj_dirs) = ProjectDirs::from("com", "shard", "Shard") {
            let config_dir = proj_dirs.config_dir();
            let config_path = config_dir.join("config.json");
            if let Ok(content) = fs::read_to_string(config_path) {
                if let Ok(config) = serde_json::from_str(&content) {
                    return config;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<()> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "shard", "Shard") {
            let config_dir = proj_dirs.config_dir();
            fs::create_dir_all(config_dir)?;
            let config_path = config_dir.join("config.json");
            let content = serde_json::to_string_pretty(self)?;
            fs::write(config_path, content)?;
        }
        Ok(())
    }
}

pub enum View {
    Dashboard,
    Settings,
}

pub struct ShardApp {
    pub view: View,
    pub config: AppConfig,
    pub status: String,
    pub is_running: bool,
    // Telemetry
    pub node_role: String,
    pub active_peers: usize,
    pub active_scouts: usize,
    pub tokens_generated: u64,
    pub uptime: String,
    pub reject_rate: f32,
    pub acceptance_rate: f32,
    pub nat_status: String,
    pub relay_status: String,
    pub daemon_online: bool,
    pub process_manager: Arc<Mutex<crate::process::ProcessManager>>,
    pub telemetry_rx: Option<tokio::sync::mpsc::Receiver<TelemetryUpdate>>,
    // Tray (owned here so it lives on the main thread, satisfying !Send on macOS)
    tray_manager: TrayManager,
    // Signals from tray menu events
    pub show_signal: Arc<AtomicBool>,
    pub quit_signal: Arc<AtomicBool>,
    pub pause_signal: Arc<AtomicBool>,
}

#[derive(Clone, Debug)]
pub struct TelemetryUpdate {
    pub role: String,
    pub peers: usize,
    pub active_scouts: usize,
    pub tokens: u64,
    pub uptime: String,
    pub reject_rate: f32,
    pub nat_status: String,
    pub relay_status: String,
    pub daemon_online: bool,
}

impl ShardApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        pm: Arc<Mutex<crate::process::ProcessManager>>,
        rx: tokio::sync::mpsc::Receiver<TelemetryUpdate>,
        tray_manager: TrayManager,
        show_signal: Arc<AtomicBool>,
        quit_signal: Arc<AtomicBool>,
        pause_signal: Arc<AtomicBool>,
    ) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.window_rounding = 8.0.into();
        cc.egui_ctx.set_visuals(visuals);

        let config = AppConfig::load();

        let started = if config.auto_start {
            let binary = find_daemon_binary();
            pm.lock().unwrap().start(&binary, DAEMON_ARGS).is_ok()
        } else {
            false
        };

        Self {
            view: View::Dashboard,
            status: if started {
                "Starting...".to_string()
            } else {
                "Idle".to_string()
            },
            is_running: started,
            node_role: "Scout".to_string(),
            active_peers: 0,
            active_scouts: 0,
            tokens_generated: 0,
            uptime: "00:00:00".to_string(),
            reject_rate: 0.0,
            acceptance_rate: 0.0,
            nat_status: "unknown".to_string(),
            relay_status: "unknown".to_string(),
            daemon_online: false,
            process_manager: pm,
            telemetry_rx: Some(rx),
            tray_manager,
            show_signal,
            quit_signal,
            pause_signal,
            config,
        }
    }

    fn poll_telemetry(&mut self) {
        if let Some(rx) = &mut self.telemetry_rx {
            while let Ok(update) = rx.try_recv() {
                self.node_role = update.role;
                self.active_peers = update.peers;
                self.active_scouts = update.active_scouts;
                self.tokens_generated = update.tokens;
                self.uptime = update.uptime;
                self.reject_rate = update.reject_rate;
                self.acceptance_rate = 1.0 - update.reject_rate;
                self.nat_status = update.nat_status;
                self.relay_status = update.relay_status;
                self.daemon_online = update.daemon_online;
                if update.daemon_online && self.is_running {
                    self.status = "Contributing".to_string();
                }
            }
        }
    }

    fn update_tray_tooltip(&self) {
        let tooltip = if self.daemon_online {
            format!(
                "Shard Node \u{2014} Contributing ({} peers)",
                self.active_peers
            )
        } else if self.is_running {
            "Shard Node \u{2014} Starting...".to_string()
        } else {
            "Shard Node".to_string()
        };
        self.tray_manager.update_tooltip(&tooltip);
    }
}

impl eframe::App for ShardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_telemetry();
        self.update_tray_tooltip();
        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        // Close → minimize to tray instead of quitting
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        // Act on tray signals
        if self.show_signal.swap(false, Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        if self.quit_signal.swap(false, Relaxed) {
            self.process_manager.lock().unwrap().stop().ok();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        if self.pause_signal.swap(false, Relaxed) {
            if self.is_running {
                self.process_manager.lock().unwrap().stop().ok();
                self.is_running = false;
                self.daemon_online = false;
                self.status = "Idle".to_string();
            } else {
                let binary = find_daemon_binary();
                if self
                    .process_manager
                    .lock().unwrap()
                    .start(&binary, DAEMON_ARGS)
                    .is_ok()
                {
                    self.is_running = true;
                    self.status = "Starting...".to_string();
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(matches!(self.view, View::Dashboard), "Dashboard")
                    .clicked()
                {
                    self.view = View::Dashboard;
                }
                if ui
                    .selectable_label(matches!(self.view, View::Settings), "Settings")
                    .clicked()
                {
                    self.view = View::Settings;
                }
            });

            ui.separator();

            match self.view {
                View::Dashboard => self.ui_dashboard(ui),
                View::Settings => self.ui_settings(ui),
            }
        });
    }
}

impl ShardApp {
    fn ui_dashboard(&mut self, ui: &mut egui::Ui) {
        // Status banner
        let (dot_color, status_label) = if self.daemon_online {
            (egui::Color32::from_rgb(0x22, 0xC5, 0x5E), "Contributing")
        } else if self.is_running {
            (egui::Color32::from_rgb(0xEA, 0xB3, 0x08), "Starting...")
        } else {
            (egui::Color32::from_rgb(0xEF, 0x44, 0x44), "Stopped")
        };

        ui.horizontal(|ui| {
            ui.colored_label(dot_color, "\u{25CF}"); // filled circle ●
            ui.label(egui::RichText::new(status_label).strong().color(dot_color));
        });

        ui.add_space(10.0);

        // Start / Stop controls
        ui.horizontal(|ui| {
            if self.is_running {
                if ui.button("Stop Node").clicked() {
                    let mut pm = self.process_manager.lock().unwrap();
                    if pm.stop().is_ok() {
                        self.is_running = false;
                        self.daemon_online = false;
                        self.status = "Idle".to_string();
                    }
                }
            } else if ui.button("Start Node").clicked() {
                let binary = find_daemon_binary();
                let mut pm = self.process_manager.lock().unwrap();
                if pm.start(&binary, DAEMON_ARGS).is_ok() {
                    self.is_running = true;
                    self.status = "Starting...".to_string();
                }
            }
        });

        ui.add_space(16.0);

        // Stats grid
        egui::Grid::new("metrics_grid")
            .num_columns(2)
            .spacing([40.0, 10.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Connected Peers:");
                ui.label(self.active_peers.to_string());
                ui.end_row();

                ui.label("Active Scouts:");
                ui.label(self.active_scouts.to_string());
                ui.end_row();

                ui.label("Tokens Generated:");
                ui.label(self.tokens_generated.to_string());
                ui.end_row();

                ui.label("Acceptance Rate:");
                ui.label(format!("{:.1}%", self.acceptance_rate * 100.0));
                ui.end_row();

                ui.label("Uptime:");
                ui.label(&self.uptime);
                ui.end_row();

                ui.label("NAT Status:");
                ui.label(&self.nat_status);
                ui.end_row();
            });
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Node Settings");
        ui.add_space(10.0);

        egui::Grid::new("settings_grid")
            .num_columns(2)
            .spacing([20.0, 10.0])
            .show(ui, |ui| {
                ui.label("BitNet Library Path:");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.config.bitnet_lib_path);
                    if ui.button("Browse...").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                            self.config.bitnet_lib_path = path.display().to_string();
                        }
                    }
                });
                ui.end_row();

                ui.label("BitNet Model (.gguf):");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.config.bitnet_model_path);
                    if ui.button("Browse...").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                            self.config.bitnet_model_path = path.display().to_string();
                        }
                    }
                });
                ui.end_row();

                ui.label("Max RAM (GB):");
                ui.add(egui::Slider::new(&mut self.config.max_ram_gb, 1.0..=64.0));
                ui.end_row();

                ui.label("Max VRAM (GB):");
                ui.add(egui::Slider::new(&mut self.config.max_vram_gb, 0.0..=24.0));
                ui.end_row();

                ui.label("Listen Port:");
                ui.add(egui::DragValue::new(&mut self.config.listen_port).range(1024..=65535));
                ui.end_row();

                ui.label("Auto-start on launch:");
                ui.checkbox(&mut self.config.auto_start, "");
                ui.end_row();
            });

        ui.add_space(20.0);
        if ui.button("Save Settings").clicked() {
            let _ = self.config.save();
        }
    }
}
