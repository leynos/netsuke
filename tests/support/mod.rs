//! Test utilities for process management.

use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tempfile::{NamedTempFile, TempDir};

fn write_script(dir: &TempDir, body: &str) -> PathBuf {
    let path = dir.path().join("ninja");
    let mut file = File::create(&path).expect("script");
    file.write_all(body.as_bytes()).expect("write script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("perms");
    }
    path
}

/// Create a fake Ninja executable that exits with `exit_code`.
///
/// Returns the temporary directory and the path to the executable.
pub fn fake_ninja(exit_code: i32) -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("temp dir");
    let body = format!("#!/bin/sh\nexit {exit_code}\n");
    let path = write_script(&dir, &body);
    (dir, path)
}

/// Create a fake Ninja that copies the provided build file to a capture path.
///
/// Returns the temporary directory, path to the executable and the capture file
/// location.
#[allow(dead_code, reason = "not every test exercises the capture variant")]
pub fn fake_ninja_capture() -> (TempDir, PathBuf, PathBuf) {
    let dir = TempDir::new().expect("temp dir");
    let capture = dir.path().join("captured.ninja");
    let body = format!(
        "#!/bin/sh\nwhile [ \"$1\" != \"\" ]; do\n  if [ \"$1\" = \"-f\" ]; then\n    shift\n    cat \"$1\" > \"{}\"\n  fi\n  shift\ndone\n",
        capture.display()
    );
    let path = write_script(&dir, &body);
    (dir, path, capture)
}

/// Write a minimal Netsukefile to the provided temporary file.
#[allow(
    dead_code,
    reason = "helper is unused when the support crate builds independently"
)]
pub fn write_manifest(file: &mut NamedTempFile) {
    let manifest = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: out\n",
        "    recipe:\n",
        "      kind: command\n",
        "      command: echo hi\n",
        "defaults:\n",
        "  - out\n",
    );
    file.write_all(manifest.as_bytes()).expect("write manifest");
}
