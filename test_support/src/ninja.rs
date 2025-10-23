//! Helpers for working with the system `ninja` binary in integration tests.

use std::fmt;
use std::process::{Command, ExitStatus};
use tempfile::{TempDir, tempdir};

/// Errors that can occur when preparing Ninja-backed integration tests.
#[derive(Debug)]
pub enum NinjaWorkspaceError {
    /// The `ninja --version` probe failed to spawn, most likely because Ninja
    /// is not present in `PATH`.
    ProbeSpawn(std::io::Error),
    /// `ninja --version` executed but returned a non-success status.
    ProbeFailed(ExitStatus),
    /// Creating the temporary workspace failed.
    Workspace(std::io::Error),
}

impl fmt::Display for NinjaWorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProbeSpawn(err) => {
                write!(f, "failed to spawn `ninja --version`: {err}")
            }
            Self::ProbeFailed(status) => {
                write!(f, "`ninja --version` exited with status {status}")
            }
            Self::Workspace(err) => write!(f, "failed to create ninja workspace: {err}"),
        }
    }
}

impl std::error::Error for NinjaWorkspaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ProbeSpawn(err) => Some(err),
            Self::ProbeFailed(_) => None,
            Self::Workspace(err) => Some(err),
        }
    }
}

fn probe_ninja() -> Result<(), NinjaWorkspaceError> {
    let output = Command::new("ninja")
        .arg("--version")
        .output()
        .map_err(NinjaWorkspaceError::ProbeSpawn)?;

    if !output.status.success() {
        return Err(NinjaWorkspaceError::ProbeFailed(output.status));
    }
    Ok(())
}

/// Ensure Ninja is available and return a temporary directory for integration
/// tests. Callers should skip their scenario when this returns `Err`.
pub fn ninja_integration_workspace() -> Result<TempDir, NinjaWorkspaceError> {
    probe_ninja()?;
    tempdir().map_err(NinjaWorkspaceError::Workspace)
}
