//! Shared workspace and fake-Ninja helpers for logging integration tests.

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
#[cfg(unix)]
use cap_std::fs::PermissionsExt;
use cap_std::{ambient_authority, fs_utf8::Dir};
use netsuke::runner::NINJA_ENV;
use rstest::fixture;
use std::path::Path;
use tempfile::{TempDir, tempdir};

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

#[cfg(not(unix))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "the fallible signature must match the Unix variant so the shared call site needs no platform-specific handling"
)]
const fn make_script_executable(_dir: &Dir, _path: &Utf8Path) -> Result<()> {
    Ok(())
}

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

pub(super) fn write_fake_ninja_script(
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
        .with_context(|| format!("write fake ninja script {path}"))?;
    make_script_executable(dir, path)
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
    ensure!(output.status.success(), "expected verbose build to succeed");
    String::from_utf8(output.stderr).context("stderr should be valid UTF-8")
}
