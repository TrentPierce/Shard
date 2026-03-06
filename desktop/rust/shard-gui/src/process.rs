/// Manages the in-process daemon running on a dedicated OS thread with its own
/// Tokio runtime. This avoids the `Send` requirement that `tokio::spawn` imposes,
/// since the daemon future holds raw C pointers across await points.
pub struct DaemonTask {
    thread: Option<std::thread::JoinHandle<()>>,
    stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl DaemonTask {
    pub fn new() -> Self {
        Self {
            thread: None,
            stop_tx: None,
        }
    }

    pub fn start(&mut self, bitnet_lib: &str, bitnet_model: &str, listen_port: u16) {
        self.stop();

        // Resolve the engine library path:
        // 1. Use whatever the user explicitly configured.
        // 2. Fall back to shard_engine.dll bundled next to this executable.
        let resolved_lib = if !bitnet_lib.is_empty() {
            bitnet_lib.to_string()
        } else {
            Self::bundled_engine_path()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default()
        };

        if !resolved_lib.is_empty() {
            std::env::set_var("BITNET_LIB", &resolved_lib);
        }
        if !bitnet_model.is_empty() {
            std::env::set_var("BITNET_MODEL", bitnet_model);
        }
        // If either piece is still missing, run as Scout (no compute contribution).
        if resolved_lib.is_empty() || bitnet_model.is_empty() {
            std::env::set_var("SHARD_REQUIRE_ENGINE_FOR_CONTRIBUTE", "false");
        }

        let args = vec![
            "shard-desktop".to_string(),
            "--control-port".to_string(),
            "9091".to_string(),
            "--tcp-port".to_string(),
            listen_port.to_string(),
            "--webrtc-port".to_string(),
            "9090".to_string(),
            "--quic-port".to_string(),
            "9092".to_string(),
        ];

        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        self.stop_tx = Some(stop_tx);

        self.thread = Some(
            std::thread::Builder::new()
                .name("shard-daemon".to_string())
                .spawn(move || {
                    let rt = tokio::runtime::Builder::new_multi_thread()
                        .enable_all()
                        .build()
                        .expect("daemon tokio runtime");
                    // block_on does not require the future to be Send, so the
                    // daemon's non-Send future (raw C pointers in ShardInitConfig)
                    // runs fine here.
                    rt.block_on(shard_daemon::run_until_stopped(args, stop_rx))
                        .ok();
                })
                .expect("daemon thread"),
        );
    }

    pub fn stop(&mut self) {
        // Sending () (or dropping sender) causes run_until_stopped to return.
        drop(self.stop_tx.take());
        // Drop the thread handle — the daemon thread will exit shortly.
        drop(self.thread.take());
    }

    /// Returns the path to `shard_engine.dll` bundled next to this executable,
    /// if it exists. Returns `None` if not found (e.g. dev build without DLLs).
    fn bundled_engine_path() -> Option<std::path::PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let dir = exe.parent()?;
        #[cfg(target_os = "windows")]
        let lib_name = "shard_engine.dll";
        #[cfg(target_os = "macos")]
        let lib_name = "libshard_engine.dylib";
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let lib_name = "libshard_engine.so";
        let candidate = dir.join(lib_name);
        if candidate.exists() {
            Some(candidate)
        } else {
            None
        }
    }
}

impl Drop for DaemonTask {
    fn drop(&mut self) {
        self.stop();
    }
}
