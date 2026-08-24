//! Helpers for working with the system `ninja` binary in integration tests.

use mockable::{DefaultEnv, Env};
use std::process::{Command, ExitStatus, Stdio};
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
    /// `ninja --version` did not exit before the timeout elapsed.
    #[error("`ninja --version` timed out after {0:?}")]
    ProbeTimeout(Duration),
    /// Creating the temporary workspace failed.
    #[error("failed to create temporary ninja workspace: {0}")]
    Workspace(#[source] std::io::Error),
}

/// Best-effort cleanup of a child process that has timed out. Attempts to kill the process and
/// wait for it to exit, ignoring any errors.
fn cleanup_timed_out_child(child: &mut std::process::Child) {
    drop(child.kill());
    drop(child.wait());
}

/// Probe `ninja --version`, timing out after two seconds.
///
/// # Errors
///
/// Returns an error if the probe cannot spawn, exits unsuccessfully, or times
/// out.
fn probe_ninja() -> Result<(), NinjaWorkspaceError> {
    let mut child = Command::new("ninja")
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
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
                    cleanup_timed_out_child(&mut child);
                    return Err(NinjaWorkspaceError::ProbeTimeout(timeout));
                }
                thread::sleep(poll_sleep);
            }
            Err(err) => return Err(NinjaWorkspaceError::ProbeSpawn(err)),
        }
    }
}

fn ninja_is_required(env: &impl Env) -> bool {
    env.os_string("NETSUKE_REQUIRE_NINJA")
        .is_some_and(|value| value == "1")
}

fn report_required_ninja_unavailable(error: &NinjaWorkspaceError) -> ! {
    panic!("Ninja is required for this test run: {error}");
}

fn probe_ninja_with_requirement(env: &impl Env) -> Result<(), NinjaWorkspaceError> {
    match probe_ninja() {
        Ok(()) => Ok(()),
        Err(error) if ninja_is_required(env) => report_required_ninja_unavailable(&error),
        Err(error) => Err(error),
    }
}
/// Ensure Ninja is available and return a temporary directory for integration
/// tests. Callers may skip their scenario when this returns `Err`, unless CI
/// has set `NETSUKE_REQUIRE_NINJA=1`.
///
/// # Errors
///
/// Returns an error if Ninja is unavailable or the integration workspace cannot be created.
///
/// # Panics
///
/// Panics when an unavailable Ninja binary is required by the injected
/// `NETSUKE_REQUIRE_NINJA=1` test-run setting.
pub fn ninja_integration_workspace() -> Result<TempDir, NinjaWorkspaceError> {
    probe_ninja_with_requirement(&DefaultEnv)?;
    tempdir().map_err(NinjaWorkspaceError::Workspace)
}
