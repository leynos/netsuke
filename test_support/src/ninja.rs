//! Helpers for working with the system `ninja` binary in integration tests.

use std::process::{Command, ExitStatus};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::{TempDir, tempdir};
use thiserror::Error;

/// Errors that can occur when preparing Ninja-backed integration tests.
#[derive(Error, Debug)]
pub enum NinjaWorkspaceError {
    /// The `ninja --version` probe failed to spawn, most likely because Ninja
    /// is not present in `PATH`.
    #[error("failed to spawn `ninja --version`: {0}")]
    ProbeSpawn(#[source] std::io::Error),
    /// `ninja --version` executed but returned a non-success status.
    #[error("`ninja --version` returned non-success status: {0}")]
    ProbeFailed(ExitStatus),
    /// Creating the temporary workspace failed.
    #[error("failed to create temporary ninja workspace: {0}")]
    Workspace(#[source] std::io::Error),
}

fn probe_ninja() -> Result<(), NinjaWorkspaceError> {
    let mut child = Command::new("ninja")
        .arg("--version")
        .spawn()
        .map_err(NinjaWorkspaceError::ProbeSpawn)?;

    let timeout = Duration::from_secs(2);
    let poll_sleep = Duration::from_millis(50);
    let start = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(());
                }
                return Err(NinjaWorkspaceError::ProbeFailed(status));
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let status = child.wait().map_err(NinjaWorkspaceError::ProbeSpawn)?;
                    return Err(NinjaWorkspaceError::ProbeFailed(status));
                }
                thread::sleep(poll_sleep);
            }
            Err(err) => return Err(NinjaWorkspaceError::ProbeSpawn(err)),
        }
    }
}

/// Ensure Ninja is available and return a temporary directory for integration
/// tests. Callers should skip their scenario when this returns `Err`.
pub fn ninja_integration_workspace() -> Result<TempDir, NinjaWorkspaceError> {
    probe_ninja()?;
    tempdir().map_err(NinjaWorkspaceError::Workspace)
}
