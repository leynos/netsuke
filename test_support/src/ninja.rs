//! Helpers for working with the system `ninja` binary in integration tests.

use mockable::{DefaultEnv, Env};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
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

/// Resolve `ninja` to the first executable file on the ambient `PATH`.
///
/// Returning the absolute path lets a caller probe and then launch the same
/// executable. Resolving relative `PATH` entries against the current directory
/// matters because the caller may hand the path to a child process that runs
/// somewhere else. `None` means no executable named `ninja` was found, which is
/// how a missing installation is reported to the callers that skip on it.
#[must_use]
pub fn resolve_ninja_program() -> Option<PathBuf> {
    let search: OsString = DefaultEnv.os_string("PATH")?;
    let current_dir = std::env::current_dir().ok()?;
    std::env::split_paths(&search)
        .map(|directory| {
            if directory.is_absolute() {
                directory
            } else {
                current_dir.join(directory)
            }
        })
        .map(|directory| directory.join("ninja"))
        .find(|candidate| is_executable_file(candidate))
}

/// Return whether `path` is a regular file the current user may execute.
///
/// The mode check mirrors `execvp`, which skips a `PATH` entry holding a file
/// without an execute bit rather than failing, so the resolved path is the one
/// a bare `ninja` would have run. The probe stays inside `test_support::fs`,
/// which is this crate's ambient filesystem boundary.
#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    crate::fs::is_executable_file(path)
}

/// Return whether `path` is a regular file the launcher could run.
///
/// There is no execute bit to inspect here, so a regular file is the whole
/// test. A bare `ninja` may still resolve through a platform suffix that this
/// join does not add; [`probe_ninja`] keeps that fallback.
#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

/// Probe `ninja --version`, timing out after two seconds.
///
/// The probe runs the path [`resolve_ninja_program`] returned when it found
/// one, so a test that later pins that path through `NETSUKE_NINJA` probes the
/// executable it will actually launch. When nothing is found the bare name is
/// tried, which fails with the spawn error that reports a missing installation.
///
/// # Errors
///
/// Returns an error if the probe cannot spawn, exits unsuccessfully, or times
/// out.
fn probe_ninja() -> Result<(), NinjaWorkspaceError> {
    let program = resolve_ninja_program().unwrap_or_else(|| PathBuf::from("ninja"));
    let mut child = Command::new(program)
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

/// Return whether the current test run requires Ninja to be installed.
fn ninja_is_required(env: &impl Env) -> bool {
    env.os_string("NETSUKE_REQUIRE_NINJA")
        .is_some_and(|value| value == "1")
}

/// Abort when a required Ninja installation is unavailable.
fn report_required_ninja_unavailable(error: &NinjaWorkspaceError) -> ! {
    panic!("Ninja is required for this test run: {error}");
}

/// Probe Ninja and escalate an unavailable required installation.
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
