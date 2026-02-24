use anyhow::Result;
use std::process::{Child, Command, Stdio};
use tracing::{error, info};

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
}

impl Drop for ProcessManager {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
