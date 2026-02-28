//! Terminal Core Tool
//!
//! Native execution of shell commands. Unlike WASM plugins, this runs directly
//! on the host OS with the same privileges as the Rove daemon. Execution
//! must be carefully guarded.

use anyhow::Result;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct TerminalTool {
    work_dir: String,
    timeout: Duration,
}

impl TerminalTool {
    pub fn new(work_dir: String) -> Self {
        Self {
            work_dir,
            timeout: Duration::from_secs(60), // Default 60s timeout
        }
    }

    /// Execute a shell command
    ///
    /// The command is run via `sh -c` on Unix or `cmd /C` on Windows.
    pub async fn execute(&self, command: &str) -> Result<String> {
        info!("Executing terminal command: {}", command);

        #[cfg(target_os = "windows")]
        let mut cmd = Command::new("cmd");
        #[cfg(target_os = "windows")]
        cmd.arg("/C").arg(command);

        #[cfg(not(target_os = "windows"))]
        let mut cmd = Command::new("sh");
        #[cfg(not(target_os = "windows"))]
        cmd.arg("-c").arg(command);

        cmd.current_dir(&self.work_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let result = tokio::time::timeout(self.timeout, cmd.output()).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                if output.status.success() {
                    debug!("Command succeeded: {}", stdout);
                    if stdout.is_empty() && !stderr.is_empty() {
                        Ok(stderr)
                    } else {
                        Ok(stdout)
                    }
                } else {
                    let err_msg = format!(
                        "Command failed with status: {}\nStdout: {}\nStderr: {}",
                        output.status, stdout, stderr
                    );
                    warn!("{}", err_msg);
                    Err(anyhow::anyhow!(err_msg))
                }
            }
            Ok(Err(e)) => {
                let err_msg = format!("Failed to start command: {}", e);
                warn!("{}", err_msg);
                Err(anyhow::anyhow!(err_msg))
            }
            Err(_) => {
                let err_msg = format!("Command timed out after {} seconds", self.timeout.as_secs());
                warn!("{}", err_msg);
                Err(anyhow::anyhow!(err_msg))
            }
        }
    }
}
