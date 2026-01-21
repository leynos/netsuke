//! Test-support crate for Netsuke.
//!
//! This crate provides test-only utilities for:
//! - creating fake executables for process-related tests
//! - manipulating PATH safely (PathGuard)
//! - serializing environment mutation across tests (EnvLock)
//! - computing SHA-256 hashes for cache keys (hash module)
//! - spawning lightweight HTTP servers for network tests (http module)
//!
//! All items are intended for use in tests within this workspace; avoid using
//! them in production code.
//!
//! Platform notes: fake executables are implemented for Unix and Windows.

pub mod check_ninja;
pub mod command_helper;
pub mod env;
pub mod env_guard;
pub mod env_lock;
pub mod env_var_guard;
pub mod exec;
pub mod hash;
pub mod http;
pub mod localizer;
pub mod manifest;
pub mod netsuke;
pub mod ninja;
pub mod path_guard;
pub mod stdlib_assert;
/// Re-export the SHA-256 helper for concise call sites.
pub use hash::sha256_hex;
/// Re-export of [`PathGuard`] for crate-level ergonomics in tests.
pub use path_guard::PathGuard;

/// Re-export of [`env_var_guard::EnvVarGuard`] for ergonomics in tests.
pub use env_var_guard::EnvVarGuard;

/// Re-export of the generic environment guard utilities.
pub use env_guard::{EnvGuard, Environment, StdEnv};

/// Re-export localizer helpers for integration tests.
pub use localizer::{localizer_test_lock, set_en_localizer};

/// Re-export manifest helpers for integration tests.
pub use manifest::ensure_manifest_exists;

/// Helpers for writing executable stubs and setting executable bits in tests.
pub use exec::{make_executable, write_exec};

mod error;
/// Format an error and its sources (outermost → root) using `Display`, joined
/// with ": ", to produce deterministic text for test assertions.
pub use error::display_error_chain;

use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Errors returned when probing for required binaries on the test host.
#[derive(Debug)]
pub enum ProbesError {
    /// One or more probes failed; each string describes the failing command.
    Failures(Vec<String>),
}

impl std::fmt::Display for ProbesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProbesError::Failures(failures) => write!(
                f,
                "Required binaries missing or failing: {}",
                failures.join(", ")
            ),
        }
    }
}

impl std::error::Error for ProbesError {}

/// Create a fake Ninja executable that exits with `exit_code`.
///
/// Returns the temporary directory and the path to the executable.
///
/// The returned [`TempDir`] must be kept alive for the executable to remain on
/// disk.
///
/// # Example
///
/// ```rust,ignore
/// use test_support::fake_ninja;
///
/// // Create a fake `ninja` that exits with code 1
/// let (dir, ninja_path) = fake_ninja(1u8);
///
/// // Prepend `dir.path()` to PATH via your env helper, then spawn `ninja`.
/// // When `dir` is dropped, the fake executable is removed.
/// ```
pub fn fake_ninja(exit_code: u8) -> Result<(TempDir, PathBuf)> {
    let dir = TempDir::new().context("fake_ninja: create temporary directory")?;

    #[cfg(unix)]
    let path = dir.path().join("ninja");
    #[cfg(windows)]
    let path = dir.path().join("ninja.cmd");

    #[cfg(unix)]
    {
        let mut file = File::create(&path)
            .with_context(|| format!("fake_ninja: create script {}", path.display()))?;
        writeln!(file, "#!/bin/sh\nexit {}", exit_code)
            .with_context(|| format!("fake_ninja: write script {}", path.display()))?;
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)
            .with_context(|| format!("fake_ninja: read metadata {}", path.display()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms)
            .with_context(|| format!("fake_ninja: set permissions {}", path.display()))?;
    }

    #[cfg(windows)]
    {
        let mut file = File::create(&path)
            .with_context(|| format!("fake_ninja: create batch file {}", path.display()))?;
        writeln!(file, "@echo off\r\nexit /B {}", exit_code)
            .with_context(|| format!("fake_ninja: write batch file {}", path.display()))?;
    }

    Ok((dir, path))
}

/// Probe that required binaries are available in `PATH`.
///
/// Each entry provides the programme name and the arguments used to probe it,
/// typically `["--version"]`. The function returns `Ok(())` when every command
/// spawns and exits successfully. Failures yield `Err` containing
/// human-readable descriptions so callers can surface an appropriate skip
/// message.
///
/// # Examples
///
/// ```rust,no_run
/// use test_support::ensure_binaries_available;
///
/// if let Err(err) = ensure_binaries_available(&[("ninja", &["--version"])]) {
///     eprintln!("skipping test: {err}");
/// }
/// ```
pub fn ensure_binaries_available(probes: &[(&str, &[&str])]) -> Result<(), ProbesError> {
    let mut failures = Vec::new();

    for (program, args) in probes {
        let probe = Command::new(program).args(*args).output();
        match probe {
            Ok(output) if output.status.success() => {}
            Ok(output) => failures.push(format!(
                "`{program}` exited with status {status}",
                status = output.status
            )),
            Err(err) => failures.push(format!("failed to spawn `{program}`: {err}")),
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(ProbesError::Failures(failures))
    }
}
