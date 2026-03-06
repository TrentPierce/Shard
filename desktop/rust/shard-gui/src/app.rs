use anyhow::Result;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::autostart;
use crate::log_capture::LogBuffer;
use crate::model_download::DownloadMsg;
use crate::process::DaemonTask;
use crate::tray::TrayManager;

// ─── Palette ────────────────────────────────────────────────────────────────

const BLUE: egui::Color32 = egui::Color32::from_rgb(0x0E, 0xA5, 0xE9);
const GREEN: egui::Color32 = egui::Color32::from_rgb(0x22, 0xC5, 0x5E);
const YELLOW: egui::Color32 = egui::Color32::from_rgb(0xEA, 0xB3, 0x08);
const RED: egui::Color32 = egui::Color32::from_rgb(0xEF, 0x44, 0x44);
const MUTED: egui::Color32 = egui::Color32::from_rgb(0x64, 0x74, 0x8B);
const PANEL_BG: egui::Color32 = egui::Color32::from_rgb(0x0D, 0x14, 0x22);
const LOG_TEXT: egui::Color32 = egui::Color32::from_rgb(0xCB, 0xD5, 0xE1);
const LOG_WARN: egui::Color32 = egui::Color32::from_rgb(0xFB, 0xD3, 0x8D);
const LOG_ERROR: egui::Color32 = egui::Color32::from_rgb(0xF8, 0x71, 0x71);
const LOG_DIM: egui::Color32 = egui::Color32::from_rgb(0x47, 0x55, 0x69);
const POST_DOWNLOAD_RESTART_MAX_ATTEMPTS: u8 = 2;
const POST_DOWNLOAD_RETRY_DELAY: Duration = Duration::from_secs(8);

// ─── Config ──────────────────────────────────────────────────────────────────

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
        proj_dirs()
            .and_then(|d| fs::read_to_string(d.config_dir().join("config.json")).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        if let Some(dirs) = proj_dirs() {
            fs::create_dir_all(dirs.config_dir())?;
            fs::write(
                dirs.config_dir().join("config.json"),
                serde_json::to_string_pretty(self)?,
            )?;
        }
        Ok(())
    }
}

fn proj_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "shard", "Shard")
}

// ─── Download state ───────────────────────────────────────────────────────────

pub enum DownloadState {
    Idle,
    InProgress {
        downloaded: u64,
        total: u64,
        filename: String,
    },
    Done,
    /// Error message, plus whether the user has dismissed the banner.
    Error(String, bool),
}

// ─── Telemetry ────────────────────────────────────────────────────────────────

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
    pub credits: i64,
    pub receipts: usize,
    pub wallet: String,
}

// ─── View ────────────────────────────────────────────────────────────────────

pub enum View {
    Dashboard,
    Settings,
}

// ─── App ─────────────────────────────────────────────────────────────────────

pub struct ShardApp {
    pub view: View,
    pub config: AppConfig,
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
    pub compute_credits: i64,
    pub receipts_count: usize,
    pub node_wallet: String,
    // Download
    pub download_state: DownloadState,
    pub download_rx: Option<tokio::sync::mpsc::Receiver<DownloadMsg>>,
    post_download_restart_attempts: u8,
    post_download_next_retry: Option<Instant>,
    // Channels
    pub daemon_task: Arc<Mutex<DaemonTask>>,
    pub telemetry_rx: Option<tokio::sync::mpsc::Receiver<TelemetryUpdate>>,
    pub log_buffer: LogBuffer,
    // Tray
    tray_manager: TrayManager,
    pub show_signal: Arc<AtomicBool>,
    pub quit_signal: Arc<AtomicBool>,
    pub pause_signal: Arc<AtomicBool>,
}

impl ShardApp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        daemon_task: Arc<Mutex<DaemonTask>>,
        telemetry_rx: tokio::sync::mpsc::Receiver<TelemetryUpdate>,
        download_rx: tokio::sync::mpsc::Receiver<DownloadMsg>,
        log_buffer: LogBuffer,
        tray_manager: TrayManager,
        show_signal: Arc<AtomicBool>,
        quit_signal: Arc<AtomicBool>,
        pause_signal: Arc<AtomicBool>,
    ) -> Self {
        // Style
        let mut visuals = egui::Visuals::dark();
        visuals.window_rounding = 8.0.into();
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(0x1E, 0x29, 0x3B);
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(0x27, 0x37, 0x4D);
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(0x0E, 0xA5, 0xE9);
        visuals.panel_fill = egui::Color32::from_rgb(0x09, 0x0F, 0x1A);
        visuals.window_fill = egui::Color32::from_rgb(0x09, 0x0F, 0x1A);
        cc.egui_ctx.set_visuals(visuals);

        let config = AppConfig::load();

        // Show the spinner only when a download task is actually running
        // (main.rs only spawns the task when needs_download == true).
        let download_state = if config.bitnet_model_path.is_empty()
            || !std::path::Path::new(&config.bitnet_model_path).exists()
        {
            DownloadState::InProgress {
                downloaded: 0,
                total: 0,
                filename: "Fetching manifest…".to_string(),
            }
        } else {
            DownloadState::Idle
        };

        let started = if config.auto_start {
            daemon_task.lock().unwrap().start(
                &config.bitnet_lib_path,
                &config.bitnet_model_path,
                config.listen_port,
            );
            true
        } else {
            false
        };

        Self {
            view: View::Dashboard,
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
            compute_credits: 0,
            receipts_count: 0,
            node_wallet: String::new(),
            download_state,
            download_rx: Some(download_rx),
            post_download_restart_attempts: 0,
            post_download_next_retry: None,
            daemon_task,
            telemetry_rx: Some(telemetry_rx),
            log_buffer,
            tray_manager,
            show_signal,
            quit_signal,
            pause_signal,
            config,
        }
    }

    fn poll_telemetry(&mut self) {
        if let Some(rx) = &mut self.telemetry_rx {
            while let Ok(u) = rx.try_recv() {
                self.node_role = u.role;
                self.active_peers = u.peers;
                self.active_scouts = u.active_scouts;
                self.tokens_generated = u.tokens;
                self.uptime = u.uptime;
                self.reject_rate = u.reject_rate;
                self.acceptance_rate = 1.0 - u.reject_rate;
                self.nat_status = u.nat_status;
                self.relay_status = u.relay_status;
                self.daemon_online = u.daemon_online;
                self.compute_credits = u.credits;
                self.receipts_count = u.receipts;
                if !u.wallet.is_empty() {
                    self.node_wallet = u.wallet;
                }
            }
        }
    }

    fn poll_download(&mut self) {
        let mut msgs = Vec::new();
        if let Some(rx) = &mut self.download_rx {
            while let Ok(msg) = rx.try_recv() {
                msgs.push(msg);
            }
        }
        for msg in msgs {
            match msg {
                DownloadMsg::Progress {
                    downloaded,
                    total,
                    filename,
                } => {
                    self.download_state = DownloadState::InProgress {
                        downloaded,
                        total,
                        filename,
                    };
                }
                DownloadMsg::Done(path) => {
                    self.config.bitnet_model_path = path.display().to_string();
                    let _ = self.config.save();
                    if let Err(error) =
                        crate::cleanup::run_startup_cleanup(self.config.bitnet_model_path.as_str())
                    {
                        tracing::warn!(error = %error, "post-download cleanup skipped");
                    }
                    // Restart daemon with the freshly downloaded model.
                    self.start_daemon();
                    self.post_download_restart_attempts = 0;
                    self.post_download_next_retry = Some(Instant::now() + POST_DOWNLOAD_RETRY_DELAY);
                    self.download_state = DownloadState::Done;
                    drop(self.download_rx.take()); // channel no longer needed
                }
                DownloadMsg::Error(e) => {
                    self.download_state = DownloadState::Error(e, false);
                    drop(self.download_rx.take());
                }
            }
        }
    }

    fn update_tray(&self) {
        let tooltip = if self.daemon_online {
            format!("Shard — {} peers", self.active_peers)
        } else if self.is_running {
            "Shard — starting…".to_string()
        } else {
            "Shard — stopped".to_string()
        };
        self.tray_manager.update_tooltip(&tooltip);
    }

    /// If the post-download daemon restart failed silently, retry a couple of times
    /// so users do not have to click "Save & Restart Node" manually.
    fn recover_post_download_startup(&mut self) {
        let Some(next_retry) = self.post_download_next_retry else {
            return;
        };

        if !matches!(self.download_state, DownloadState::Done) || self.daemon_online {
            self.post_download_next_retry = None;
            return;
        }
        if !self.is_running {
            // User intentionally stopped the node.
            self.post_download_next_retry = None;
            return;
        }
        if Instant::now() < next_retry {
            return;
        }
        if self.post_download_restart_attempts >= POST_DOWNLOAD_RESTART_MAX_ATTEMPTS {
            self.post_download_next_retry = None;
            return;
        }

        self.post_download_restart_attempts += 1;
        self.start_daemon();
        self.post_download_next_retry = Some(Instant::now() + POST_DOWNLOAD_RETRY_DELAY);
    }
}

// ─── eframe::App ─────────────────────────────────────────────────────────────

impl eframe::App for ShardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_telemetry();
        self.poll_download();
        self.recover_post_download_startup();
        self.update_tray();
        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        // Close → minimize to tray
        if ctx.input(|i| i.viewport().close_requested()) && !self.quit_signal.load(Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        if self.show_signal.swap(false, Relaxed) {
            self.view = View::Dashboard;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        if self.quit_signal.swap(false, Relaxed) {
            self.daemon_task.lock().unwrap().stop();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.pause_signal.swap(false, Relaxed) {
            self.toggle_node();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            self.ui_root(ui);
        });
    }
}

// ─── UI ──────────────────────────────────────────────────────────────────────

impl ShardApp {
    fn ui_root(&mut self, ui: &mut egui::Ui) {
        // Header bar
        self.ui_header(ui);
        ui.add_space(2.0);

        // Tab row
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            let dash_sel = matches!(self.view, View::Dashboard);
            let sett_sel = matches!(self.view, View::Settings);
            if tab_button(ui, "Dashboard", dash_sel).clicked() {
                self.view = View::Dashboard;
            }
            ui.add_space(4.0);
            if tab_button(ui, "Settings", sett_sel).clicked() {
                self.view = View::Settings;
            }
        });

        ui.add(egui::Separator::default().spacing(8.0));

        match self.view {
            View::Dashboard => self.ui_dashboard(ui),
            View::Settings => self.ui_settings(ui),
        }
    }

    fn ui_header(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(0x0D, 0x14, 0x22))
            .inner_margin(egui::Margin::symmetric(16.0, 10.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Status dot + label
                    let (dot, label) = self.status_display();
                    ui.colored_label(dot, "●");
                    ui.label(egui::RichText::new(label).strong().size(15.0).color(dot));

                    // Role chip
                    if self.daemon_online {
                        ui.add_space(6.0);
                        role_chip(ui, &self.node_role);
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        self.ui_toggle_button(ui);
                    });
                });
            });
    }

    fn ui_toggle_button(&mut self, ui: &mut egui::Ui) {
        if self.is_running {
            let btn =
                egui::Button::new(egui::RichText::new("Stop Node").color(egui::Color32::WHITE))
                    .fill(egui::Color32::from_rgb(0x7F, 0x1D, 0x1D))
                    .rounding(6.0)
                    .min_size(egui::vec2(90.0, 28.0));
            if ui.add(btn).clicked() {
                self.daemon_task.lock().unwrap().stop();
                self.is_running = false;
                self.daemon_online = false;
                self.post_download_next_retry = None;
            }
        } else {
            let btn =
                egui::Button::new(egui::RichText::new("Start Node").color(egui::Color32::WHITE))
                    .fill(egui::Color32::from_rgb(0x06, 0x5F, 0x46))
                    .rounding(6.0)
                    .min_size(egui::vec2(90.0, 28.0));
            if ui.add(btn).clicked() {
                self.start_daemon();
            }
        }
    }

    fn status_display(&self) -> (egui::Color32, &'static str) {
        match &self.download_state {
            DownloadState::InProgress { .. } => (BLUE, "Downloading model…"),
            _ => {
                if self.daemon_online {
                    (GREEN, "Contributing")
                } else if self.is_running {
                    (YELLOW, "Starting…")
                } else {
                    (RED, "Stopped")
                }
            }
        }
    }

    fn start_daemon(&mut self) {
        let mut task = self.daemon_task.lock().unwrap();
        task.start(
            &self.config.bitnet_lib_path,
            &self.config.bitnet_model_path,
            self.config.listen_port,
        );
        self.is_running = true;
        self.daemon_online = false;
    }

    fn toggle_node(&mut self) {
        if self.is_running {
            self.daemon_task.lock().unwrap().stop();
            self.is_running = false;
            self.daemon_online = false;
            self.post_download_next_retry = None;
        } else {
            self.start_daemon();
        }
    }

    // ── Dashboard ────────────────────────────────────────────────────────────

    fn ui_dashboard(&mut self, ui: &mut egui::Ui) {
        let log_height = 200.0_f32.min(ui.available_height() * 0.38);
        let stats_height = ui.available_height() - log_height - 12.0;

        ui.allocate_ui(egui::vec2(ui.available_width(), stats_height), |ui| {
            // Dismissable error banner (shown above stats, not replacing them)
            if let DownloadState::Error(msg, dismissed) = &self.download_state {
                if !dismissed {
                    let msg = msg.clone();
                    self.ui_download_error_banner(ui, &msg);
                }
            }

            match &self.download_state {
                DownloadState::InProgress {
                    downloaded,
                    total,
                    filename,
                } => {
                    self.ui_download(ui, *downloaded, *total, filename.clone());
                }
                _ => {
                    self.ui_stats(ui);
                }
            }
        });

        ui.add_space(4.0);
        ui.separator();
        self.ui_log_panel(ui, log_height);
    }

    fn ui_download(&self, ui: &mut egui::Ui, downloaded: u64, total: u64, filename: String) {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("Downloading AI Model")
                    .size(16.0)
                    .strong()
                    .color(BLUE),
            );
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new("Shard requires a BitNet model to contribute compute.")
                    .color(MUTED),
            );
            ui.add_space(20.0);

            let frame_w = (ui.available_width() - 60.0).max(200.0);
            ui.allocate_ui(egui::vec2(frame_w, 80.0), |ui| {
                ui.label(egui::RichText::new(&filename).monospace().color(LOG_TEXT));
                ui.add_space(8.0);

                let progress = if total > 0 {
                    downloaded as f32 / total as f32
                } else {
                    0.0
                };
                let bar = egui::ProgressBar::new(progress)
                    .desired_width(frame_w)
                    .animate(true);
                ui.add(bar);

                ui.add_space(6.0);
                if total > 0 {
                    ui.label(
                        egui::RichText::new(format!(
                            "{:.1} / {:.1} GB  ({:.0}%)",
                            downloaded as f64 / 1e9,
                            total as f64 / 1e9,
                            progress * 100.0,
                        ))
                        .color(MUTED)
                        .size(12.0),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("Fetching manifest…")
                            .color(MUTED)
                            .size(12.0),
                    );
                }
            });
        });
    }

    fn ui_download_error_banner(&mut self, ui: &mut egui::Ui, error: &str) {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(0x3D, 0x1A, 0x08))
            .rounding(6.0)
            .inner_margin(egui::Margin::symmetric(12.0, 8.0))
            .outer_margin(egui::Margin::symmetric(8.0, 4.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(YELLOW, "⚠");
                    ui.label(egui::RichText::new(error).color(YELLOW).size(12.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("✕").clicked() {
                            if let DownloadState::Error(_, dismissed) = &mut self.download_state {
                                *dismissed = true;
                            }
                        }
                    });
                });
            });
    }

    fn ui_stats(&self, ui: &mut egui::Ui) {
        ui.add_space(12.0);

        let col_w = (ui.available_width() / 2.0 - 24.0).max(180.0);

        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // Left column — Node Stats
            ui.vertical(|ui| {
                ui.set_min_width(col_w);
                section_label(ui, "Node Stats");
                ui.add_space(6.0);
                stat_row(ui, "Peers", &self.active_peers.to_string());
                stat_row(ui, "Active Scouts", &self.active_scouts.to_string());
                stat_row(ui, "Uptime", &self.uptime);
                stat_row(ui, "NAT", &self.nat_status);
                stat_row(ui, "Relay", &self.relay_status);
            });

            ui.add_space(24.0);
            ui.separator();
            ui.add_space(24.0);

            // Right column — Contribution
            ui.vertical(|ui| {
                ui.set_min_width(col_w);
                section_label(ui, "Contribution");
                ui.add_space(6.0);
                stat_row(ui, "Tokens", &fmt_large(self.tokens_generated));
                stat_row(ui, "Receipts", &self.receipts_count.to_string());
                stat_row(ui, "Compute Credits", &self.compute_credits.to_string());
                stat_row(
                    ui,
                    "Acceptance Rate",
                    &format!("{:.1}%", self.acceptance_rate * 100.0),
                );
                if !self.node_wallet.is_empty() {
                    let short = wallet_short(&self.node_wallet);
                    stat_row(ui, "Wallet", &short);
                }
            });
        });
    }

    // ── Log panel ────────────────────────────────────────────────────────────

    fn ui_log_panel(&self, ui: &mut egui::Ui, height: f32) {
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(egui::RichText::new("Logs").strong().color(MUTED).size(12.0));
        });
        ui.add_space(2.0);

        egui::Frame::none()
            .fill(PANEL_BG)
            .rounding(6.0)
            .inner_margin(egui::Margin::symmetric(10.0, 6.0))
            .outer_margin(egui::Margin::symmetric(8.0, 0.0))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height(height)
                    .stick_to_bottom(true)
                    .id_salt("log_scroll")
                    .show(ui, |ui| {
                        if let Ok(lines) = self.log_buffer.lock() {
                            if lines.is_empty() {
                                ui.label(
                                    egui::RichText::new("No log output yet…")
                                        .monospace()
                                        .size(11.0)
                                        .color(LOG_DIM),
                                );
                            }
                            for line in lines.iter() {
                                let color = log_line_color(line);
                                ui.label(
                                    egui::RichText::new(line)
                                        .monospace()
                                        .size(11.0)
                                        .color(color),
                                );
                            }
                        }
                    });
            });
    }

    // ── Settings ─────────────────────────────────────────────────────────────

    fn ui_settings(&mut self, ui: &mut egui::Ui) {
        ui.add_space(12.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(4.0);
            egui::Frame::none()
                .inner_margin(egui::Margin::symmetric(16.0, 8.0))
                .show(ui, |ui| {
                    egui::Grid::new("settings_grid")
                        .num_columns(2)
                        .spacing([24.0, 12.0])
                        .show(ui, |ui| {
                            settings_header(ui, "Engine  (optional — Scout mode works without these)");
                            ui.end_row();

                            ui.label("BitNet Library (.dll/.so):")
                                .on_hover_text("Path to the compiled BitNet inference library.\nObtain from your Shard distribution or build from source.\nLeave blank to run as Scout (network relay only).");
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut self.config.bitnet_lib_path);
                                if ui.small_button("Browse").clicked() {
                                    if let Some(p) = rfd::FileDialog::new().pick_file() {
                                        self.config.bitnet_lib_path = p.display().to_string();
                                    }
                                }
                            });
                            ui.end_row();

                            ui.label("Model (.gguf):")
                                .on_hover_text("Path to the BitNet GGUF model file.\nDownloaded automatically on first launch when available.\nLeave blank to run as Scout.");
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut self.config.bitnet_model_path);
                                if ui.small_button("Browse").clicked() {
                                    if let Some(p) = rfd::FileDialog::new().pick_file() {
                                        self.config.bitnet_model_path = p.display().to_string();
                                    }
                                }
                            });
                            ui.end_row();

                            ui.end_row(); // spacer

                            settings_header(ui, "Resources");
                            ui.end_row();

                            ui.label("Max RAM (GB):");
                            ui.add(egui::Slider::new(&mut self.config.max_ram_gb, 1.0..=128.0));
                            ui.end_row();

                            ui.label("Max VRAM (GB):");
                            ui.add(egui::Slider::new(&mut self.config.max_vram_gb, 0.0..=48.0));
                            ui.end_row();

                            ui.label("Listen Port:");
                            ui.add(
                                egui::DragValue::new(&mut self.config.listen_port)
                                    .range(1024..=65535),
                            );
                            ui.end_row();

                            ui.end_row(); // spacer

                            settings_header(ui, "Startup");
                            ui.end_row();

                            ui.label("Start daemon on app launch:");
                            ui.checkbox(&mut self.config.auto_start, "");
                            ui.end_row();

                            ui.label("Launch at login:");
                            let mut autostart_on = autostart::is_autostart_enabled();
                            if ui.checkbox(&mut autostart_on, "").changed() {
                                let _ = autostart::set_autostart(autostart_on);
                            }
                            ui.end_row();
                        });

                    ui.add_space(20.0);

                    ui.horizontal(|ui| {
                        let save_btn = egui::Button::new(
                            egui::RichText::new("Save & Restart Node").color(egui::Color32::WHITE),
                        )
                        .fill(BLUE)
                        .rounding(6.0)
                        .min_size(egui::vec2(160.0, 30.0));
                        if ui.add(save_btn).clicked() {
                            let _ = self.config.save();
                            self.start_daemon();
                            self.view = View::Dashboard;
                        }
                    });
                });
        });
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn tab_button(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let fill = if selected {
        egui::Color32::from_rgb(0x1E, 0x40, 0x5A)
    } else {
        egui::Color32::TRANSPARENT
    };
    let text_color = if selected { BLUE } else { MUTED };
    let btn = egui::Button::new(egui::RichText::new(label).color(text_color).size(13.0))
        .fill(fill)
        .rounding(5.0)
        .min_size(egui::vec2(80.0, 26.0));
    ui.add(btn)
}

fn role_chip(ui: &mut egui::Ui, role: &str) {
    let (fill, text) = match role {
        "Shard" => (egui::Color32::from_rgb(0x05, 0x3A, 0x26), "⬡ Shard"),
        _ => (egui::Color32::from_rgb(0x1E, 0x2A, 0x40), "◎ Scout"),
    };
    egui::Frame::none()
        .fill(fill)
        .rounding(4.0)
        .inner_margin(egui::Margin::symmetric(6.0, 2.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(11.0).color(BLUE));
        });
}

fn section_label(ui: &mut egui::Ui, label: &str) {
    ui.label(egui::RichText::new(label).strong().size(13.0).color(BLUE));
    ui.add(egui::Separator::default().spacing(4.0));
}

fn stat_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).color(MUTED).size(12.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value).strong().size(13.0));
        });
    });
    ui.add_space(2.0);
}

fn settings_header(ui: &mut egui::Ui, label: &str) {
    ui.label(egui::RichText::new(label).strong().color(MUTED));
    ui.label(""); // second column filler
}

fn log_line_color(line: &str) -> egui::Color32 {
    let t = line.trim_start();
    if t.starts_with("ERROR") || t.contains(" ERROR ") {
        LOG_ERROR
    } else if t.starts_with("WARN") || t.contains(" WARN ") {
        LOG_WARN
    } else if t.starts_with("DEBUG") || t.starts_with("TRACE") {
        LOG_DIM
    } else {
        LOG_TEXT
    }
}

fn fmt_large(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn wallet_short(wallet: &str) -> String {
    if wallet.len() > 12 {
        format!("{}…{}", &wallet[..6], &wallet[wallet.len() - 4..])
    } else {
        wallet.to_string()
    }
}
