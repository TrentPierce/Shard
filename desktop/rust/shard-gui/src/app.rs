use anyhow::Result;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Arc;
use tokio::sync::Mutex;

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
            auto_start: false,
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
    pub tokens_generated: u64,
    pub uptime: String,
    pub vram_alloc_gb: f32,
    pub vram_limit_gb: f32,
    pub reject_rate: f32,
    pub nat_status: String,
    pub relay_status: String,
    pub process_manager: Arc<Mutex<crate::process::ProcessManager>>,
    pub telemetry_rx: Option<tokio::sync::mpsc::Receiver<TelemetryUpdate>>,
    pub download_progress: Option<f32>,
    pub is_downloading: bool,
}

#[derive(Clone, Debug)]
pub struct TelemetryUpdate {
    pub role: String,
    pub peers: usize,
    pub tokens: u64,
    pub uptime: String,
    pub vram_alloc_gb: f32,
    pub vram_limit_gb: f32,
    pub reject_rate: f32,
    pub nat_status: String,
    pub relay_status: String,
}

impl ShardApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        pm: Arc<Mutex<crate::process::ProcessManager>>,
        rx: tokio::sync::mpsc::Receiver<TelemetryUpdate>,
    ) -> Self {
        // Customize look
        let mut visuals = egui::Visuals::dark();
        visuals.window_rounding = 8.0.into();
        cc.egui_ctx.set_visuals(visuals);

        Self {
            view: View::Dashboard,
            config: AppConfig::load(),
            status: "Idle".to_string(),
            is_running: false,
            node_role: "Scout".to_string(),
            active_peers: 0,
            tokens_generated: 0,
            uptime: "00:00:00".to_string(),
            vram_alloc_gb: 0.0,
            vram_limit_gb: 0.0,
            reject_rate: 0.0,
            nat_status: "unknown".to_string(),
            relay_status: "unknown".to_string(),
            process_manager: pm,
            telemetry_rx: Some(rx),
            download_progress: None,
            is_downloading: false,
        }
    }

    fn poll_telemetry(&mut self) {
        if let Some(rx) = &mut self.telemetry_rx {
            while let Ok(update) = rx.try_recv() {
                self.node_role = update.role;
                self.active_peers = update.peers;
                self.tokens_generated = update.tokens;
                self.uptime = update.uptime;
                self.vram_alloc_gb = update.vram_alloc_gb;
                self.vram_limit_gb = update.vram_limit_gb;
                self.reject_rate = update.reject_rate;
                self.nat_status = update.nat_status;
                self.relay_status = update.relay_status;
            }
        }
    }

    fn ensure_assets(&mut self) -> bool {
        let models_dir = if let Some(proj_dirs) = ProjectDirs::from("com", "shard", "Shard") {
            let data_dir = proj_dirs.data_dir();
            let models_dir = data_dir.join("models");
            fs::create_dir_all(&models_dir).ok();
            models_dir
        } else {
            return false;
        };

        let default_model_filename = "Llama-3.2-1B-Instruct-Q4_K_M.gguf";
        let model_path = models_dir.join(default_model_filename);

        if !model_path.exists() {
            if !self.is_downloading {
                self.start_download(
                    "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf",
                    model_path.clone()
                );
            }
            return false;
        }

        if self.config.bitnet_model_path.is_empty() {
            self.config.bitnet_model_path = model_path.display().to_string();
            let _ = self.config.save();
        }

        true
    }

    fn start_download(&mut self, url: &str, dest: std::path::PathBuf) {
        self.is_downloading = true;
        self.status = "Downloading assets...".to_string();
        self.download_progress = Some(0.0);

        // In a real implementation, we'd use a more sophisticated download task
        // for now, we'll simulate it or use reqwest in a thread
        let url = url.to_string();
        tokio::spawn(async move {
            if let Ok(response) = reqwest::get(url).await {
                if let Ok(bytes) = response.bytes().await {
                    let _ = fs::write(dest, bytes);
                }
            }
        });
    }
}

impl eframe::App for ShardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_telemetry();
        ctx.request_repaint_after(std::time::Duration::from_millis(500));

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
        ui.heading("Node Dashboard");
        ui.add_space(10.0);

        if self.is_downloading {
            ui.vertical_centered(|ui| {
                ui.label("Downloading default model (Llama-3.2-1B)...");
                if let Some(p) = self.download_progress {
                    ui.add(egui::ProgressBar::new(p).show_percentage());
                } else {
                    ui.spinner();
                }
                ui.label("This may take a few minutes depending on your connection.");
            });
            ui.add_space(10.0);

            // Simple check to see if download finished
            if let Some(proj_dirs) = ProjectDirs::from("com", "shard", "Shard") {
                let model_path = proj_dirs
                    .data_dir()
                    .join("models")
                    .join("Llama-3.2-1B-Instruct-Q4_K_M.gguf");
                if model_path.exists() {
                    self.is_downloading = false;
                    self.status = "Idle".to_string();
                    self.config.bitnet_model_path = model_path.display().to_string();
                    let _ = self.config.save();
                }
            }
        }

        ui.horizontal(|ui| {
            if self.is_running {
                if ui.button("Stop Node").clicked() {
                    let mut pm = self.process_manager.blocking_lock();
                    if pm.stop().is_ok() {
                        self.is_running = false;
                        self.status = "Idle".to_string();
                    }
                }
            } else if ui.button("Start Node").clicked() && self.ensure_assets() {
                let mut pm = self.process_manager.blocking_lock();
                // Example: start the daemon
                if pm
                    .start("shard-daemon.exe", &["--control-port", "9091"])
                    .is_ok()
                {
                    self.is_running = true;
                    self.status = "Running".to_string();
                }
            }
            ui.label(format!("Status: {}", self.status));
        });

        ui.add_space(20.0);

        egui::Grid::new("metrics_grid")
            .num_columns(2)
            .spacing([40.0, 10.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Node Role:");
                ui.label(&self.node_role);
                ui.end_row();

                ui.label("Active Mesh Peers:");
                ui.label(self.active_peers.to_string());
                ui.end_row();

                ui.label("Tokens Generated:");
                ui.label(self.tokens_generated.to_string());
                ui.end_row();

                ui.label("Uptime:");
                ui.label(&self.uptime);
                ui.end_row();

                ui.label("VRAM Alloc (GB):");
                if self.vram_limit_gb > 0.0 {
                    ui.label(format!("{:.2} / {:.2}", self.vram_alloc_gb, self.vram_limit_gb));
                } else {
                    ui.label(format!("{:.2}", self.vram_alloc_gb));
                }
                ui.end_row();

                ui.label("Reject Rate:");
                ui.label(format!("{:.1}%", self.reject_rate * 100.0));
                ui.end_row();

                ui.label("NAT Status:");
                ui.label(&self.nat_status);
                ui.end_row();

                ui.label("Relay Status:");
                ui.label(&self.relay_status);
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
            });

        ui.add_space(20.0);
        if ui.button("Save Settings").clicked() {
            let _ = self.config.save();
        }
    }
}
