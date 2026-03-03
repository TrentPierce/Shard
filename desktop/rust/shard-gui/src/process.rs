use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use tracing::{error, info};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

pub const DAEMON_ARGS: &[&str] = &[
    "--control-port",
    "9091",
    "--tcp-port",
    "4001",
    "--webrtc-port",
    "9090",
    "--quic-port",
    "9092",
];

pub fn find_daemon_binary() -> PathBuf {
    #[cfg(target_os = "windows")]
    let binary_name = "shard-daemon.exe";
    #[cfg(not(target_os = "windows"))]
    let binary_name = "shard-daemon";

    // Check directory of own executable first
    if let Ok(mut exe_path) = std::env::current_exe() {
        exe_path.pop(); // Remove filename, keep dir
        let candidate = exe_path.join(binary_name);
        if candidate.exists() {
            return candidate;
        }
    }

    // Fall back to PATH
    PathBuf::from(binary_name)
}

pub struct ProcessManager {
    child: Option<Child>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self { child: None }
    }

    pub fn start(&mut self, program: &Path, args: &[&str]) -> Result<()> {
        if self.child.is_some() {
            self.stop()?;
        }

        info!("Starting child process: {:?} {:?}", program, args);

        let mut cmd = Command::new(program);
        cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

        let child = cmd.spawn()?;
        self.child = Some(child);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            info!("Stopping child process (PID: {:?})", child.id());
            match child.kill() {
                Ok(_) => {
                    let _ = child.wait(); // Prevent zombies
                    info!("Child process terminated.");
                }
                Err(e) => {
                    error!("Failed to kill child process: {}", e);
                    return Err(e.into());
                }
            }
        }
        Ok(())
    }
}

impl Drop for ProcessManager {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
