//! Ambient filesystem helpers for test fixtures.
//!
//! Netsuke production code accesses the filesystem through capability-scoped
//! `cap_std` handles, enforced by Whitaker's `no_std_fs_operations` lint.
//! Test fixtures, however, routinely stage workspaces in ambient temporary
//! directories where a capability handle adds ceremony without isolation
//! value. This module confines that ambient access to `test_support`, which
//! `dylint.toml` excludes from the lint — the same pattern Whitaker itself
//! uses for its `whitaker_common` test utilities.
//!
//! Scope and reuse policy: test fixtures and assertions only; production code
//! must keep using `cap_std`.

use std::fs;
use std::io;
use std::path::Path;

/// Write `contents` to `path`, creating or truncating the file.
///
/// # Errors
///
/// Propagates the underlying `std::fs::write` failure.
///
/// # Examples
///
/// ```
/// let dir = tempfile::tempdir().expect("create tempdir");
/// let path = dir.path().join("fixture.txt");
/// test_support::fs::write(&path, "hello").expect("write fixture");
/// ```
pub fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
    fs::write(path, contents)
}

/// Read the entire contents of `path` as bytes.
///
/// # Errors
///
/// Propagates the underlying `std::fs::read` failure.
pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    fs::read(path)
}

/// Read the entire contents of `path` as a UTF-8 string.
///
/// # Errors
///
/// Propagates the underlying `std::fs::read_to_string` failure.
pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    fs::read_to_string(path)
}

/// Create a single directory at `path`.
///
/// # Errors
///
/// Propagates the underlying `std::fs::create_dir` failure.
pub fn create_dir(path: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir(path)
}

/// Create `path` and any missing parent directories.
///
/// # Errors
///
/// Propagates the underlying `std::fs::create_dir_all` failure.
pub fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(path)
}

/// Remove the file at `path`.
///
/// # Errors
///
/// Propagates the underlying `std::fs::remove_file` failure.
pub fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
    fs::remove_file(path)
}

/// Return `true` when `path` exists (following symlinks).
///
/// # Examples
///
/// ```
/// let dir = tempfile::tempdir().expect("create tempdir");
/// assert!(test_support::fs::exists(dir.path()));
/// assert!(!test_support::fs::exists(dir.path().join("absent")));
/// ```
#[must_use]
pub fn exists(path: impl AsRef<Path>) -> bool {
    fs::metadata(path).is_ok()
}

/// Return the length in bytes of the file at `path`.
///
/// # Errors
///
/// Propagates the underlying metadata failure.
pub fn file_len(path: impl AsRef<Path>) -> io::Result<u64> {
    Ok(fs::metadata(path)?.len())
}

/// Set the Unix permission bits on `path` (for example `0o755` or `0o000`).
///
/// # Errors
///
/// Propagates metadata or permission-change failures.
#[cfg(unix)]
pub fn set_mode(path: impl AsRef<Path>, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let path = path.as_ref();
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions)
}
