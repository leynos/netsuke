//! Write small fake Ninja scripts for integration tests.
//!
//! This module owns script formatting and executable permissions for tests
//! that need deterministic Ninja status lines. It is limited to stdout lines
//! and one optional stderr marker; fixtures that create artefacts or model
//! richer process behaviour should keep that logic at their owning test
//! boundary. Production code must not use this module.

use anyhow::{Context, Result};
use camino::Utf8Path;
#[cfg(unix)]
use cap_std::fs::PermissionsExt;
use cap_std::fs_utf8::Dir;

/// Write a status-emitting fake Ninja script inside a capability-scoped directory.
///
/// The script echoes each `stdout_lines` entry to standard output, optionally
/// writes `stderr_marker` to standard error, and exits successfully. Unix
/// scripts receive the executable bit; Windows uses a batch file.
///
/// # Examples
///
/// ```
/// use camino::Utf8Path;
/// use cap_std::{ambient_authority, fs_utf8::Dir};
/// use tempfile::tempdir;
///
/// let temporary = tempdir()?;
/// let path = Utf8Path::from_path(temporary.path())
///     .ok_or_else(|| std::io::Error::other("temporary path should be UTF-8"))?;
/// let directory = Dir::open_ambient_dir(path, ambient_authority())?;
/// let script_name = if cfg!(windows) { "ninja.cmd" } else { "ninja" };
/// test_support::ninja_script::write_fake_ninja_script(
///     &directory,
///     Utf8Path::new(script_name),
///     &["[1/1] echo ready"],
///     Some("fixture complete"),
/// )?;
/// assert!(test_support::fs::exists(temporary.path().join(script_name)));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns an error if the script cannot be written or, on Unix, its
/// executable permission cannot be set.
pub fn write_fake_ninja_script(
    dir: &Dir,
    path: &Utf8Path,
    stdout_lines: &[&str],
    stderr_marker: Option<&str>,
) -> Result<()> {
    let script = if cfg!(windows) {
        let mut script = String::from("@echo off\r\n");
        for line in stdout_lines {
            script.push_str("echo ");
            script.push_str(line);
            script.push_str("\r\n");
        }
        if let Some(marker) = stderr_marker {
            script.push_str("echo ");
            script.push_str(marker);
            script.push_str(" 1>&2\r\n");
        }
        script.push_str("exit /B 0\r\n");
        script
    } else {
        let mut script = String::from(
            "#!/bin/sh\nwhile IFS= read -r line; do\n  printf '%s\\n' \"$line\"\ndone <<'NETSUKE_OUTPUT'\n",
        );
        for line in stdout_lines {
            script.push_str(line);
            script.push('\n');
        }
        script.push_str("NETSUKE_OUTPUT\n");
        if let Some(marker) = stderr_marker {
            script.push_str("printf '%s\\n' '");
            script.push_str(marker);
            script.push_str("' >&2\n");
        }
        script.push_str("exit 0\n");
        script
    };

    dir.write(path, script)
        .with_context(|| format!("write fake Ninja script {path}"))?;
    make_script_executable(dir, path)
}

/// Set the executable bit on a fake Ninja script where the platform supports it.
///
/// # Examples
///
/// ```ignore
/// make_script_executable(&directory, Utf8Path::new("ninja"))?;
/// ```
#[cfg(unix)]
fn make_script_executable(dir: &Dir, path: &Utf8Path) -> Result<()> {
    let mut permissions = dir
        .metadata(path)
        .with_context(|| format!("read metadata for {path}"))?
        .permissions();
    permissions.set_mode(0o755);
    dir.set_permissions(path, permissions)
        .with_context(|| format!("set executable bit for {path}"))?;
    Ok(())
}

/// Leave fake Ninja permissions unchanged on platforms without executable bits.
///
/// # Examples
///
/// ```ignore
/// make_script_executable(&directory, Utf8Path::new("ninja"))?;
/// ```
///
/// The operation returns `Ok(())` without changing file permissions.
#[cfg(not(unix))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "the signature matches the Unix implementation, whose permission change can fail"
)]
const fn make_script_executable(_dir: &Dir, _path: &Utf8Path) -> Result<()> {
    Ok(())
}
