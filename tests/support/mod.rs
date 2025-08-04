//! Test utilities for process management.

use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

/// Create a fake Ninja executable that exits with `exit_code`.
///
/// Returns the temporary directory and the path to the executable.
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
