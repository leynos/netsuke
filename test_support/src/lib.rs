//! Test-only utilities for integration and unit tests.
//!
//! This crate offers helpers for crafting fake executables, manipulating the
//! environment, and guarding global state so tests can exercise process
//! interactions without touching the host system.

pub mod check_ninja;
pub mod env;
pub mod env_lock;
pub mod path_guard;

pub use path_guard::PathGuard;

use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

/// Create a fake Ninja executable that exits with `exit_code`.
///
/// Returns the temporary directory and the path to the executable.
pub fn fake_ninja(exit_code: i32) -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("failed to create temporary directory");

    #[cfg(unix)]
    let path = dir.path().join("ninja");
    #[cfg(windows)]
    let path = dir.path().join("ninja.cmd");

    #[cfg(unix)]
    {
        let mut file = File::create(&path).expect("failed to create script");
        writeln!(file, "#!/bin/sh\nexit {}", exit_code).expect("failed to write script");
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)
            .expect("failed to read script metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("failed to set script permissions");
    }

    #[cfg(windows)]
    {
        let mut file = File::create(&path).expect("failed to create batch file");
        writeln!(file, "@echo off\r\nexit /B {}", exit_code).expect("failed to write batch file");
    }

    (dir, path)
}

// Additional helpers can be added here as the test suite evolves.
