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
pub mod env_var_guard;
pub mod path_guard;
/// Re-export of [`PathGuard`] for crate-level ergonomics in tests.
pub use path_guard::PathGuard;

/// Re-export of [`env_var_guard::EnvVarGuard`] for ergonomics in tests.
pub use env_var_guard::EnvVarGuard;

mod error;
/// Format an error and its sources (outermost → root) using `Display`, joined
/// with ": ", to produce deterministic text for test assertions.
pub use error::display_error_chain;

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::{NamedTempFile, TempDir};

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

/// Resolve `cli_file` relative to `temp_dir` and ensure it exists.
///
/// When `cli_file` is relative, it is joined with `temp_dir` and the returned
/// [`PathBuf`] is absolute. If the resulting path does not exist, a minimal
/// manifest is written to that location.
///
/// # Panics
///
/// Panics if `temp_dir` does not exist or if any I/O error occurs while
/// creating or persisting the manifest file.
///
/// # Examples
///
/// ```rust,no_run
/// use std::path::PathBuf;
/// use tempfile::TempDir;
/// use test_support::ensure_manifest_exists;
///
/// let temp = TempDir::new().expect("temp dir");
/// let cli_file = PathBuf::from("manifest.yml");
/// let manifest = ensure_manifest_exists(temp.path(), &cli_file);
/// assert!(manifest.exists());
/// ```
pub fn ensure_manifest_exists(temp_dir: &Path, cli_file: &Path) -> PathBuf {
    let manifest_path = if cli_file.is_absolute() {
        cli_file.to_path_buf()
    } else {
        temp_dir.join(cli_file)
    };

    if !manifest_path.exists() {
        let dest_dir = manifest_path.parent().unwrap_or(temp_dir);
        if !dest_dir.exists() {
            fs::create_dir_all(dest_dir).expect(&format!(
                "Failed to create manifest parent directory for {}",
                manifest_path.display()
            ));
        }
        let mut file = NamedTempFile::new_in(dest_dir).expect(&format!(
            "Failed to create temporary manifest file for {}",
            manifest_path.display()
        ));
        crate::env::write_manifest(&mut file).expect(&format!(
            "Failed to write manifest content to {}",
            manifest_path.display()
        ));
        // Avoid clobbering an existing manifest if concurrently created.
        file.persist_noclobber(&manifest_path).expect(&format!(
            "Failed to persist manifest file to {}",
            manifest_path.display()
        ));
    }

    manifest_path
}

// Additional helpers can be added here as the test suite evolves.
