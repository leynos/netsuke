//! Helpers for invoking the built `netsuke` binary in tests.
//!
//! These utilities use `assert_cmd` to locate the current workspace's
//! `netsuke` executable and run it in a controlled working directory,
//! capturing stdout/stderr for assertions.

use anyhow::{Context, Result, ensure};
use std::path::Path;
use std::path::PathBuf;

/// Locate the built `netsuke` executable for integration-style tests.
///
/// Derive the path from the current test executable's target directory.
fn netsuke_executable() -> Result<PathBuf> {
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
/// dependency. Other process environment variables are inherited, except for
/// configuration selectors that this helper removes explicitly.
///
/// # Errors
///
/// Returns an error when `netsuke` cannot be located or the process cannot be
/// spawned.
pub fn run_netsuke_in(current_dir: &Path, args: &[&str]) -> Result<NetsukeRun> {
    let isolated_config_home = current_dir.join(".config");
    let mut cmd = assert_cmd::Command::new(netsuke_executable()?);
    let output = cmd
        .current_dir(current_dir)
        .env("PATH", "")
        .env_remove("NETSUKE_CONFIG_PATH")
        .env_remove("NETSUKE_OUTPUT_FORMAT")
        .env("HOME", current_dir)
        .env("XDG_CONFIG_HOME", &isolated_config_home)
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
/// inherits no process environment variables. The child receives only an
/// isolated `PATH`, `HOME`, `XDG_CONFIG_HOME`, and the variables supplied in
/// `extra_env`. This prevents process-level environment races when tests run
/// in parallel.
///
/// # Errors
///
/// Returns an error when `netsuke` cannot be located or the process cannot be
/// spawned.
#[expect(
    clippy::disallowed_methods,
    reason = "reads the inherited environment to build the subprocess invocation; assert_cmd subprocess isolation is the sanctioned exemption in AGENTS.md"
)]
pub fn run_netsuke_in_with_env(
    current_dir: &Path,
    args: &[&str],
    extra_env: &[(&str, &str)],
) -> Result<NetsukeRun> {
    let mut cmd = assert_cmd::Command::new(netsuke_executable()?);
    let isolated_config_home = current_dir.join(".config");
    let isolated_path = tempfile::tempdir().context("create isolated executable directory")?;
    cmd.current_dir(current_dir)
        .env_clear()
        .env("PATH", isolated_path.path())
        .env("HOME", current_dir)
        .env("XDG_CONFIG_HOME", isolated_config_home);
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
