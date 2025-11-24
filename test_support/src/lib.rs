//! Test-support crate for Netsuke.
//!
//! This crate provides test-only utilities for:
//! - creating fake executables for process-related tests
//! - manipulating PATH safely (PathGuard)
//! - serialising environment mutation across tests (EnvLock)
//! - computing SHA-256 hashes for cache keys (hash module)
//! - spawning lightweight HTTP servers for network tests (http module)
//!
//! All items are intended for use in tests within this workspace; avoid using
//! them in production code.
//!
//! Platform notes: fake executables are implemented for Unix and Windows.

pub mod check_ninja;
pub mod command_helper;
pub mod env;
pub mod env_guard;
pub mod env_lock;
pub mod env_var_guard;
pub mod hash;
pub mod http;
pub mod manifest;
pub mod ninja;
pub mod path_guard;
pub mod stdlib_assert;
/// Re-export the SHA-256 helper for concise call sites.
pub use hash::sha256_hex;
/// Re-export of [`PathGuard`] for crate-level ergonomics in tests.
pub use path_guard::PathGuard;

/// Re-export of [`env_var_guard::EnvVarGuard`] for ergonomics in tests.
pub use env_var_guard::EnvVarGuard;

/// Re-export of the generic environment guard utilities.
pub use env_guard::{EnvGuard, Environment, StdEnv};

mod error;
/// Format an error and its sources (outermost → root) using `Display`, joined
/// with ": ", to produce deterministic text for test assertions.
pub use error::display_error_chain;

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use tempfile::{NamedTempFile, TempDir};

/// Errors returned when probing for required binaries on the test host.
#[derive(Debug)]
pub enum ProbesError {
    /// One or more probes failed; each string describes the failing command.
    Failures(Vec<String>),
}

impl std::fmt::Display for ProbesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProbesError::Failures(failures) => write!(
                f,
                "Required binaries missing or failing: {}",
                failures.join(", ")
            ),
        }
    }
}

impl std::error::Error for ProbesError {}

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
pub fn fake_ninja(exit_code: u8) -> Result<(TempDir, PathBuf)> {
    let dir = TempDir::new().context("fake_ninja: create temporary directory")?;

    #[cfg(unix)]
    let path = dir.path().join("ninja");
    #[cfg(windows)]
    let path = dir.path().join("ninja.cmd");

    #[cfg(unix)]
    {
        let mut file = File::create(&path)
            .with_context(|| format!("fake_ninja: create script {}", path.display()))?;
        writeln!(file, "#!/bin/sh\nexit {}", exit_code)
            .with_context(|| format!("fake_ninja: write script {}", path.display()))?;
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)
            .with_context(|| format!("fake_ninja: read metadata {}", path.display()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms)
            .with_context(|| format!("fake_ninja: set permissions {}", path.display()))?;
    }

    #[cfg(windows)]
    {
        let mut file = File::create(&path)
            .with_context(|| format!("fake_ninja: create batch file {}", path.display()))?;
        writeln!(file, "@echo off\r\nexit /B {}", exit_code)
            .with_context(|| format!("fake_ninja: write batch file {}", path.display()))?;
    }

    Ok((dir, path))
}

/// Probe that required binaries are available in `PATH`.
///
/// Each entry provides the programme name and the arguments used to probe it,
/// typically `["--version"]`. The function returns `Ok(())` when every command
/// spawns and exits successfully. Failures yield `Err` containing
/// human-readable descriptions so callers can surface an appropriate skip
/// message.
///
/// # Examples
///
/// ```rust,no_run
/// use test_support::ensure_binaries_available;
///
/// if let Err(err) = ensure_binaries_available(&[("ninja", &["--version"])]) {
///     eprintln!("skipping test: {err}");
/// }
/// ```
pub fn ensure_binaries_available(probes: &[(&str, &[&str])]) -> Result<(), ProbesError> {
    let mut failures = Vec::new();

    for (program, args) in probes {
        let probe = Command::new(program).args(*args).output();
        match probe {
            Ok(output) if output.status.success() => {}
            Ok(output) => failures.push(format!(
                "`{program}` exited with status {status}",
                status = output.status
            )),
            Err(err) => failures.push(format!("failed to spawn `{program}`: {err}")),
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(ProbesError::Failures(failures))
    }
}

/// Resolve `cli_file` relative to `temp_dir` and ensure it exists.
///
/// When `cli_file` is relative, it is joined with `temp_dir` and the returned
/// path is absolute and UTF‑8. If the resulting path does not exist, a minimal
/// manifest is written to that location.
///
/// # Errors
///
/// Returns an [`io::Error`] if any I/O error occurs whilst validating the
/// target, creating parent directories, writing the temporary manifest, or
/// persisting it to `manifest_path`.
///
/// # Examples
///
/// ```rust,ignore
/// use camino::{Utf8Path, Utf8PathBuf};
/// use tempfile::TempDir;
/// use test_support::ensure_manifest_exists;
///
/// let temp = TempDir::new().expect("temp dir");
/// let temp_path = Utf8Path::from_path(temp.path()).expect("utf-8 path");
/// let cli_file = Utf8PathBuf::from("manifest.yml");
/// let manifest = ensure_manifest_exists(temp_path, &cli_file)
///     .expect("manifest");
/// assert!(manifest.exists());
/// ```
pub fn ensure_manifest_exists(temp_dir: &Utf8Path, cli_file: &Utf8Path) -> io::Result<Utf8PathBuf> {
    let manifest_path = resolve_manifest_path(temp_dir, cli_file)?;

    if manifest_path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            format!(
                "Manifest path points to a directory, expected a file: {}",
                manifest_path
            ),
        ));
    }

    if manifest_path.exists() {
        return Ok(manifest_path);
    }

    create_manifest_file(temp_dir, manifest_path.as_ref())?;
    Ok(manifest_path)
}

fn resolve_manifest_path(temp_dir: &Utf8Path, cli_file: &Utf8Path) -> io::Result<Utf8PathBuf> {
    let manifest_path = if cli_file.is_absolute() {
        cli_file.to_owned()
    } else {
        temp_dir.join(cli_file)
    };

    if manifest_path.file_name().is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Manifest path must include a file name: {}", manifest_path),
        ));
    }

    Ok(manifest_path)
}

fn create_manifest_file(temp_dir: &Utf8Path, manifest_path: &Utf8Path) -> io::Result<()> {
    let dest_dir = manifest_path.parent().unwrap_or(temp_dir);
    ensure_parent_directory(manifest_path, dest_dir)?;
    let mut file = create_temp_file(dest_dir, manifest_path)?;
    write_manifest_content(&mut file, manifest_path)?;
    persist_manifest_file(file, manifest_path)
}

fn create_temp_file(dest_dir: &Utf8Path, manifest_path: &Utf8Path) -> io::Result<NamedTempFile> {
    NamedTempFile::new_in(dest_dir.as_std_path()).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!(
                "Failed to create temporary manifest file for {}: {e}",
                manifest_path
            ),
        )
    })
}

fn write_manifest_content(file: &mut NamedTempFile, manifest_path: &Utf8Path) -> io::Result<()> {
    crate::env::write_manifest(file).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("Failed to write manifest content to {}: {e}", manifest_path),
        )
    })
}

fn persist_manifest_file(file: NamedTempFile, manifest_path: &Utf8Path) -> io::Result<()> {
    match file.persist(manifest_path.as_std_path()) {
        Ok(_) => Ok(()),
        Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(io::Error::new(
            e.error.kind(),
            format!(
                "Failed to persist manifest file to {} from {}: {}",
                manifest_path,
                e.file.path().display(),
                e.error
            ),
        )),
    }
}

fn ensure_parent_directory(manifest_path: &Utf8Path, dest_dir: &Utf8Path) -> io::Result<()> {
    if dest_dir.exists() {
        // If the path exists but is not a directory, report a clear error that
        // includes the final manifest path. Returning AlreadyExists mirrors the
        // semantics that the desired directory “exists” but is unusable.
        if dest_dir.is_dir() {
            return Ok(());
        }
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "Failed to create manifest parent directory for {}: parent path exists and is not a directory",
                manifest_path,
            ),
        ));
    }

    let base = find_existing_ancestor(dest_dir, manifest_path)?;

    let relative = dest_dir.strip_prefix(base).map_err(|_| {
        io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Failed to derive relative path for {} from ancestor {}",
                dest_dir, base,
            ),
        )
    })?;

    let dir = fs_utf8::Dir::open_ambient_dir(base, ambient_authority()).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!(
                "Failed to open ancestor directory {} for {}: {e}",
                base, manifest_path,
            ),
        )
    })?;

    dir.create_dir_all(relative).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!(
                "Failed to create manifest parent directory for {}: {e}",
                manifest_path,
            ),
        )
    })
}

fn find_existing_ancestor<'a>(
    dest_dir: &'a Utf8Path,
    manifest_path: &Utf8Path,
) -> io::Result<&'a Utf8Path> {
    let mut ancestors = dest_dir.ancestors();
    ancestors.next(); // Skip self

    ancestors
        .find(|candidate| candidate.exists())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "Failed to locate an existing ancestor for manifest directory {}",
                    manifest_path,
                ),
            )
        })
}

// Additional helpers can be added here as the test suite evolves.

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Context, Result, anyhow};
    use camino::Utf8Path;
    use std::fs;
    use std::io;
    use tempfile::TempDir;

    #[test]
    fn existing_directory_manifest_path_is_rejected() -> Result<()> {
        let temp = TempDir::new().context("create temp dir")?;
        let temp_path = Utf8Path::from_path(temp.path())
            .ok_or_else(|| anyhow::anyhow!("temp path is not valid UTF-8"))?;
        let dir = temp.path().join("dir");
        fs::create_dir(&dir).context("create directory placeholder")?;

        let err = ensure_manifest_exists(temp_path, Utf8Path::new("dir"))
            .expect_err("existing directory should be rejected");
        assert_eq!(err.kind(), io::ErrorKind::NotADirectory);
        let msg = err.to_string();
        let dir_str = dir
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("dir path is not valid UTF-8"))?;
        assert!(msg.contains(dir_str), "message: {msg}");
        Ok(())
    }

    #[test]
    fn read_only_parent_reports_target_path() -> Result<()> {
        let temp = TempDir::new().context("create temp dir")?;
        let temp_path = Utf8Path::from_path(temp.path())
            .ok_or_else(|| anyhow::anyhow!("temp path is not valid UTF-8"))?;
        let parent = temp.path().join("parent");
        fs::write(&parent, b"file").context("write placeholder parent file")?;
        let manifest = parent.join("manifest.yml");

        let err = ensure_manifest_exists(temp_path, Utf8Path::new("parent/manifest.yml"))
            .expect_err("non-directory parent should error");
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        let msg = err.to_string();
        let manifest_str = manifest
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("manifest path is not valid UTF-8"))?;
        assert!(msg.contains(manifest_str), "message: {msg}");
        Ok(())
    }

    #[test]
    fn creates_missing_parent_directory_and_manifest() -> Result<()> {
        let temp = TempDir::new().context("create temp dir")?;
        let temp_path = Utf8Path::from_path(temp.path())
            .ok_or_else(|| anyhow::anyhow!("temp path is not valid UTF-8"))?;

        // Parent directory does not exist beforehand.
        let cli_file = Utf8Path::new("missing/subdir/manifest.yml");
        let expected_path = temp_path.join(cli_file);
        assert!(
            !expected_path.exists(),
            "precondition: path should not exist"
        );

        let manifest_path =
            ensure_manifest_exists(temp_path, cli_file).context("create manifest when missing")?;
        assert_eq!(manifest_path, expected_path);
        assert!(manifest_path.exists(), "manifest file should exist");
        assert!(
            manifest_path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("manifest path missing parent"))?
                .exists(),
            "parent directory should be created"
        );

        // Sanity check that content was written, not an empty file.
        let contents = std::fs::read_to_string(manifest_path.as_std_path())
            .context("read manifest contents")?;
        assert!(
            contents.contains("netsuke_version:"),
            "unexpected manifest contents: {contents}"
        );
        Ok(())
    }
}
