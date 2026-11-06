//! Shared workspace and fake-Ninja helpers for logging integration tests.

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use netsuke::runner::NINJA_ENV;
use rstest::fixture;
use std::path::Path;
use tempfile::{TempDir, tempdir};
pub(super) use test_support::ninja_script::write_fake_ninja_script;

#[cfg(unix)]
use crate::command_list_failure_support::FAILURE_PREFIX;

#[fixture]
pub(super) fn temp_with_minimal_manifest() -> Result<TempDir> {
    let temp = tempdir().context("create temp dir")?;
    let workspace_path = Utf8Path::from_path(temp.path()).context("temp dir path is not UTF-8")?;
    let workspace = Dir::open_ambient_dir(workspace_path, ambient_authority())
        .context("open temporary workspace")?;
    let repository = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .context("open repository root")?;
    repository
        .copy("tests/data/minimal.yml", &workspace, "Netsukefile")
        .context("copy minimal manifest to temporary workspace")?;
    Ok(temp)
}

/// Open `temp` as a capability-scoped UTF-8 directory handle.
///
/// Mirrors the ambient-authority pattern used by [`temp_with_minimal_manifest`]
/// so tests can write workspace files through `cap_std` rather than `std::fs`.
pub(super) fn open_workspace(temp: &TempDir) -> Result<Dir> {
    let workspace_path = Utf8Path::from_path(temp.path()).context("temp dir path is not UTF-8")?;
    Dir::open_ambient_dir(workspace_path, ambient_authority())
        .with_context(|| format!("open temporary workspace {workspace_path}"))
}

#[cfg(unix)]
pub(super) struct CommandListDiagnostics<'message> {
    message: &'message str,
}

#[cfg(unix)]
impl<'message> CommandListDiagnostics<'message> {
    /// Wrap captured output for command-list diagnostic projections.
    pub(super) const fn new(message: &'message str) -> Self {
        Self { message }
    }

    /// Return the rendered human error line carrying command-list attribution.
    pub(super) fn human_error_fragment(&self) -> Option<&'message str> {
        self.message.lines().find(|line| {
            line.contains("Error:")
                && line.contains("ninja exited with")
                && line.contains(FAILURE_PREFIX)
        })
    }

    /// Return a stable human error fragment with its fingerprint replaced.
    pub(super) fn normalized_human_error_fragment(&self) -> Option<String> {
        let error = self.human_error_fragment()?;
        let diagnostic_start = error.find("ninja exited with")?;
        Some(normalize_fingerprints(error.get(diagnostic_start..)?))
    }

    /// Return a stable JSON failure marker with its fingerprint replaced.
    pub(super) fn normalized_failure_marker_fragment(&self) -> Option<String> {
        let line = self
            .message
            .lines()
            .find(|line| line.contains(FAILURE_PREFIX))?;
        let start = line.find(FAILURE_PREFIX)?;
        Some(normalize_fingerprints(line.get(start..)?))
    }

    /// Return a stable tracing action field with its fingerprint replaced.
    pub(super) fn normalized_tracing_fragment(&self) -> Option<String> {
        const ACTION_FIELD: &str = "command_list_action=";
        let line = self
            .message
            .lines()
            .find(|line| line.contains(ACTION_FIELD))?;
        let start = line.find(ACTION_FIELD)?;
        Some(normalize_fingerprints(line.get(start..)?))
    }
}

/// Replace every fixed-width hexadecimal fingerprint in a diagnostic fragment.
#[cfg(unix)]
fn normalize_fingerprints(fragment: &str) -> String {
    let bytes = fragment.as_bytes();
    let mut normalized = String::with_capacity(fragment.len());
    let mut cursor = 0;
    let mut replaced = false;

    for (start, window) in bytes.windows(64).enumerate() {
        if start < cursor || !window.iter().all(u8::is_ascii_hexdigit) {
            continue;
        }
        let Some(prefix) = fragment.get(cursor..start) else {
            return fragment.to_owned();
        };
        normalized.push_str(prefix);
        normalized.push_str("<fingerprint>");
        cursor = start + 64;
        replaced = true;
    }

    if !replaced {
        return fragment.to_owned();
    }
    let Some(suffix) = fragment.get(cursor..) else {
        return fragment.to_owned();
    };
    normalized.push_str(suffix);
    normalized
}

pub(super) fn fake_ninja_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.cmd")
    } else {
        stem.to_owned()
    }
}

pub(super) fn path_containing(dir: &Path) -> Result<std::ffi::OsString> {
    std::env::join_paths([dir]).context("build PATH containing fake ninja")
}

pub(super) fn run_verbose_build_with_ninja_env(
    current_dir: &Path,
    path_env: Option<std::ffi::OsString>,
    ninja_env: Option<&Path>,
) -> Result<String> {
    let mut command = assert_cmd::cargo::cargo_bin_cmd!("netsuke");
    command
        .current_dir(current_dir)
        .env_remove(NINJA_ENV)
        .arg("--verbose")
        .arg("build");
    if let Some(path) = path_env {
        command.env("PATH", path);
    }
    if let Some(ninja) = ninja_env {
        command.env(NINJA_ENV, ninja);
    }

    let output = command.output().context("run verbose netsuke build")?;
    ensure!(
        output.status.success(),
        "expected verbose build to succeed with status {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stderr).context("stderr should be valid UTF-8")
}
