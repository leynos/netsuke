//! Helpers for invoking the built `netsuke` binary in tests.
//!
//! These utilities use `assert_cmd` to locate the current workspace's
//! `netsuke` executable and run it in a controlled working directory,
//! capturing stdout/stderr for assertions.

use anyhow::{Context, Result, ensure};
use ninja_env::NINJA_ENV;
use std::path::Path;
use std::path::PathBuf;

/// Locate the built `netsuke` executable for integration-style tests.
///
/// Prefer `CARGO_BIN_EXE_netsuke` when available, otherwise fall back to a
/// `target/(debug|release)`-derived path based on the current test binary.
fn netsuke_executable() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_netsuke") {
        return Ok(path.into());
    }

    let mut target_dir = std::env::current_exe().context("locate current test executable")?;
    target_dir.pop();
    if target_dir.ends_with("deps") {
        target_dir.pop();
    }

    let path = target_dir.join(format!("netsuke{}", std::env::consts::EXE_SUFFIX));
    ensure!(
        path.is_file(),
        "locate netsuke binary at {}",
        path.display()
    );
    Ok(path)
}

/// Captured output from a `netsuke` invocation.
#[derive(Debug)]
pub struct NetsukeRun {
    /// Captured stdout (lossy UTF-8).
    pub stdout: String,
    /// Captured stderr (lossy UTF-8).
    pub stderr: String,
    /// Whether the command exited successfully.
    pub success: bool,
}

/// Run `netsuke` in `current_dir` with the supplied args.
///
/// The function clears `PATH` so tests don't accidentally execute a host
/// dependency. Other process environment variables are **inherited**, so
/// callers that set variables via `VarGuard` will see them forwarded.
///
/// # Errors
///
/// Returns an error when `netsuke` cannot be located or the process cannot be
/// spawned.
pub fn run_netsuke_in(current_dir: &Path, args: &[&str]) -> Result<NetsukeRun> {
    let mut cmd = assert_cmd::Command::new(netsuke_executable()?);
    let output = cmd
        .current_dir(current_dir)
        .env("PATH", "")
        .args(args)
        .output()
        .context("run netsuke command")?;
    Ok(NetsukeRun {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

/// Run `netsuke` in `current_dir` with an isolated environment.
///
/// Unlike [`run_netsuke_in`], this variant uses `env_clear()` so the child
/// process inherits **only** the variables supplied in `extra_env`. This
/// prevents process-level environment races when tests run in parallel.
///
/// `NETSUKE_NINJA` is automatically forwarded from the current process when
/// present (set by [`override_ninja_env`]), so callers that install a fake
/// ninja guard before calling this function get the expected behaviour.
///
/// [`override_ninja_env`]: crate::env::override_ninja_env
///
/// # Errors
///
/// Returns an error when `netsuke` cannot be located or the process cannot be
/// spawned.
pub fn run_netsuke_in_with_env(
    current_dir: &Path,
    args: &[&str],
    extra_env: &[(&str, &str)],
) -> Result<NetsukeRun> {
    let mut cmd = assert_cmd::Command::new(netsuke_executable()?);
    cmd.current_dir(current_dir).env_clear();
    if let Some(host_path) = std::env::var_os("PATH") {
        cmd.env("PATH", host_path);
    }
    // Forward NETSUKE_NINJA when an override_ninja_env guard is active.
    if let Some(ninja) = std::env::var_os(NINJA_ENV) {
        cmd.env(NINJA_ENV, ninja);
    }
    for &(key, value) in extra_env {
        cmd.env(key, value);
    }
    let output = cmd.args(args).output().context("run netsuke command")?;
    Ok(NetsukeRun {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}
