//! Test utilities for process management.
//!
//! This module provides helpers for creating fake executables along with
//! logging utilities used in behavioural tests.

pub mod env_lock;
pub mod path_guard;

#[expect(unused_imports, reason = "re-export for selective test crates")]
pub use path_guard::PathGuard;

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Create a fake Ninja executable that exits with `exit_code`.
///
/// Returns the temporary directory and the path to the executable.
#[allow(dead_code, reason = "used in PATH tests")]
pub fn fake_ninja(exit_code: i32) -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("ninja");
    let mut file = File::create(&path).expect("script");
    writeln!(file, "#!/bin/sh\nexit {exit_code}").expect("write script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("perms");
    }
    (dir, path)
}

/// Set up `env` so `PATH` resolves only to `dir`.
///
/// Build a valid `PATH` string that contains exactly one entry pointing to
/// `dir` and configure the mock to return it. This avoids lossy conversions
/// and makes the UTF-8 requirement explicit to callers.
///
/// Note: `MockEnv::raw` returns a `String`, so callers must accept UTF-8. This
/// helper returns an error if the constructed `PATH` cannot be represented as
/// UTF-8.
///
/// # Examples
/// ```ignore
/// let (dir, _) = fake_ninja(0);
/// let mut env = MockEnv::new();
/// mock_path_to(&mut env, dir.path());
/// ```
#[expect(dead_code, reason = "used in PATH tests")]
pub fn mock_path_to(env: &mut mockable::MockEnv, dir: &Path) -> io::Result<()> {
    // Join using the platform-appropriate separator while ensuring exactly one
    // element is present in the PATH value.
    let joined = std::env::join_paths([dir.as_os_str()])
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

    // `MockEnv::raw` expects a `String`. Propagate if the single-entry PATH is
    // not valid UTF-8 to keep the contract explicit.
    let path = joined.into_string().map_err(|os| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("non-UTF-8 PATH entry: {}", os.to_string_lossy()),
        )
    })?;

    env.expect_raw()
        .withf(|key| key == "PATH")
        .returning(move |_| Ok(path.clone()));

    Ok(())
}

/// Create a fake Ninja executable that writes its current directory to the file
/// specified as the first argument.
///
/// Returns the temporary directory and the path to the executable.
#[expect(dead_code, reason = "used only in directory tests")]
pub fn fake_ninja_pwd() -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("ninja");
    let mut file = File::create(&path).expect("script");
    // The script writes its working directory to the provided file and exits.
    writeln!(file, "#!/bin/sh\npwd > \"$1\"").expect("write script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("perms");
    }
    (dir, path)
}
