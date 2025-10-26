//! Helpers for validating build file paths via fake Ninja binaries.

use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

/// Create a fake Ninja that validates the build file path provided via `-f`.
///
/// The script exits with status `1` if the file is missing or not a regular
/// file, otherwise `0`.
pub fn fake_ninja_check_build_file() -> Result<(TempDir, PathBuf)> {
    let dir = TempDir::new().context("fake_ninja_check_build_file: create temp dir")?;
    let path = dir.path().join("ninja");
    let mut file = File::create(&path).with_context(|| {
        format!(
            "fake_ninja_check_build_file: create script {}",
            path.display()
        )
    })?;
    writeln!(
        file,
        concat!(
            "#!/bin/sh\n",
            "if [ \"$1\" = \"-f\" ] && [ ! -f \"$2\" ]; then\n",
            "  echo 'missing build file: $2' >&2\n",
            "  exit 1\n",
            "fi\n",
            "exit 0"
        ),
    )
    .with_context(|| {
        format!(
            "fake_ninja_check_build_file: write script {}",
            path.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)
            .with_context(|| {
                format!(
                    "fake_ninja_check_build_file: read metadata {}",
                    path.display()
                )
            })?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).with_context(|| {
            format!(
                "fake_ninja_check_build_file: set permissions {}",
                path.display()
            )
        })?;
    }
    Ok((dir, path))
}
