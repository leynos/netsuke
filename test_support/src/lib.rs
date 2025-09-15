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

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::PathBuf;
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
/// # Errors
///
/// Returns an [`io::Error`] if any I/O error occurs whilst creating parent
/// directories, writing the temporary manifest, or persisting it to
/// `manifest_path`.
///
/// # Examples
///
/// ```rust,no_run
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
    let manifest_path: Utf8PathBuf = if cli_file.is_absolute() {
        cli_file.to_owned()
    } else {
        temp_dir.join(cli_file)
    };

    if !manifest_path.exists() {
        if manifest_path.file_name().is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Manifest path must include a file name: {}", manifest_path),
            ));
        }

        let dest_dir = manifest_path.parent().unwrap_or(temp_dir);
        ensure_parent_directory(&manifest_path, dest_dir)?;
        let mut file = NamedTempFile::new_in(dest_dir.as_std_path()).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!(
                    "Failed to create temporary manifest file for {}: {e}",
                    manifest_path
                ),
            )
        })?;
        crate::env::write_manifest(&mut file).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("Failed to write manifest content to {}: {e}", manifest_path),
            )
        })?;
        match file.persist(manifest_path.as_std_path()) {
            Ok(_) => (),
            Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists => (),
            Err(e) => {
                return Err(io::Error::new(
                    e.error.kind(),
                    format!(
                        "Failed to persist manifest file to {} from {}: {}",
                        manifest_path,
                        e.file.path().display(),
                        e.error
                    ),
                ));
            }
        }
    }

    Ok(manifest_path)
}

<<<<<<< HEAD
fn resolve_manifest_path(temp_dir: &Path, cli_file: &Path) -> PathBuf {
    if cli_file.is_absolute() {
        cli_file.to_path_buf()
    } else {
        temp_dir.join(cli_file)
    }
}

fn ensure_directory_exists(manifest_path: &Path, temp_dir: &Path) -> io::Result<PathBuf> {
    let dest_dir = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| temp_dir.to_path_buf());

    if dest_dir.exists() { return Ok(dest_dir); }

    fs::create_dir_all(&dest_dir).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!(
                "Failed to create manifest parent directory for {}: {e}",
                manifest_path.display()
            ),
        )
    })?;

    Ok(dest_dir)
}

fn create_manifest_file(dest_dir: &Path, manifest_path: &Path) -> io::Result<NamedTempFile> {
    let file = NamedTempFile::new_in(dest_dir).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!(
                "Failed to create temporary manifest file for {}: {e}",
                manifest_path.display()
            ),
        )
    })?;
    Ok(file)
}

fn persist_manifest_file(file: NamedTempFile, manifest_path: &Path) -> io::Result<()> {
    // Avoid clobbering an existing manifest if concurrently created.
    // Treat AlreadyExists as success when another process creates it.
    match file.persist_noclobber(manifest_path) {
        Ok(_) => Ok(()),
        Err(err) if err.error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(err) => Err(io::Error::new(
            err.error.kind(),
            format!(
                "Failed to persist manifest file to {}: {}",
                manifest_path.display(),
                err.error
            ),
        )),
    }
||||||| parent of f009f57 (Use camino paths and cap-std in manifest helper)
=======
fn ensure_parent_directory(manifest_path: &Utf8Path, dest_dir: &Utf8Path) -> io::Result<()> {
    if dest_dir.exists() {
        return Ok(());
    }

    let mut ancestors = dest_dir.ancestors();
    ancestors.next();

    let base = ancestors
        .find(|candidate| candidate.exists())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "Failed to locate an existing ancestor for manifest directory {}",
                    manifest_path,
                ),
            )
        })?;

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
>>>>>>> f009f57 (Use camino paths and cap-std in manifest helper)
}

// Additional helpers can be added here as the test suite evolves.

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8Path;
    use std::fs;
    use std::io;
    use tempfile::TempDir;

fn ensure_parent_directory(manifest_path: &Utf8Path, dest_dir: &Utf8Path) -> io::Result<()> {
    if dest_dir.exists() {
        return Ok(());
    }

    let mut ancestors = dest_dir.ancestors();
    ancestors.next();

    let base = ancestors
        .find(|candidate| candidate.exists())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "Failed to locate an existing ancestor for manifest directory {}",
                    manifest_path,
                ),
            )
        })?;

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