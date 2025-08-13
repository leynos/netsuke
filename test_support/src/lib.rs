//! Test-support crate for Netsuke.
//!
//! This crate provides test-only utilities for:
//! - creating fake executables for process-related tests
//! - manipulating PATH safely (PathGuard)
//! - serialising environment mutation across tests (EnvLock)
//!
//! All items are intended for use in tests within this workspace; avoid using
//! them in production code.
//!
//! Platform notes: fake executables are implemented for Unix and Windows.

pub mod check_ninja;
pub mod env;
pub mod env_lock;
pub mod path_guard;
/// Re-export of [`PathGuard`] for crate-level ergonomics in tests.
pub use path_guard::PathGuard;

use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

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
pub fn fake_ninja(exit_code: u8) -> (TempDir, PathBuf) {
    let dir = TempDir::new()
        .unwrap_or_else(|e| panic!("fake_ninja: failed to create temporary directory: {e}"));

    #[cfg(unix)]
    let path = dir.path().join("ninja");
    #[cfg(windows)]
    let path = dir.path().join("ninja.cmd");

    #[cfg(unix)]
    {
        let mut file = File::create(&path).unwrap_or_else(|e| {
            panic!(
                "fake_ninja: failed to create script {}: {e}",
                path.display()
            )
        });
        writeln!(file, "#!/bin/sh\nexit {}", exit_code).unwrap_or_else(|e| {
            panic!("fake_ninja: failed to write script {}: {e}", path.display())
        });
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)
            .unwrap_or_else(|e| {
                panic!(
                    "fake_ninja: failed to read metadata {}: {e}",
                    path.display()
                )
            })
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap_or_else(|e| {
            panic!(
                "fake_ninja: failed to set permissions {}: {e}",
                path.display()
            )
        });
    }

    #[cfg(windows)]
    {
        let mut file = File::create(&path).unwrap_or_else(|e| {
            panic!(
                "fake_ninja: failed to create batch file {}: {e}",
                path.display()
            )
        });
        writeln!(file, "@echo off\r\nexit /B {}", exit_code).unwrap_or_else(|e| {
            panic!(
                "fake_ninja: failed to write batch file {}: {e}",
                path.display()
            )
        });
    }

    (dir, path)
}

// Additional helpers can be added here as the test suite evolves.
