use std::process::{Child, Command, Stdio};
use anyhow::Result;
use tracing::{info, error};

pub struct ProcessManager {
    child: Option<Child>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self { child: None }
    }

    pub fn start(&mut self, program: &str, args: &[&str]) -> Result<()> {
        if self.child.is_some() {
            self.stop()?;
        }

        info!("Starting child process: {} {:?}", program, args);
        let child = Command::new(program)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

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

    pub fn is_running(&self) -> bool {
        if let Some(mut child) = self.child.as_ref() {
            // Check if it's still alive
            match Command::new("tasklist")
                .arg("/FI")
                .arg(format!("PID eq {}", child.id()))
                .output() {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        stdout.contains(&child.id().to_string())
                    }
                    Err(_) => true, // Fallback to assuming it's running
                }
        } else {
            false
        }
    }
}

impl Drop for ProcessManager {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
