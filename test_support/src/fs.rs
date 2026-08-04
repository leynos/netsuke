//! Ambient filesystem helpers for test fixtures.
//!
//! Netsuke production code accesses the filesystem through capability-scoped
//! `cap_std` handles, enforced by Whitaker's `no_std_fs_operations` lint.
//! Test fixtures, however, routinely stage workspaces in ambient temporary
//! directories where a capability handle adds ceremony without isolation
//! value. This module is the crate's single ambient boundary: `test_support/`
//! carries its own `dylint.toml` naming `test_support::fs` as the only entry
//! under `[no_std_fs_operations] excluded_paths`, so a direct `std::fs` call in
//! any other module still fails the lint.
//!
//! Scope and reuse policy: test fixtures and assertions only; production code
//! must keep using `cap_std`. Other `test_support` modules that need fixture
//! I/O call through here rather than reaching for `std::fs` themselves:
//! `exec`, `manifest`, and the crate-root regression tests do so directly,
//! while `check_ninja` and `fake_ninja` reach it via
//! [`crate::exec::write_exec_with_content`].

use std::fs;
use std::io;
use std::path::Path;
use std::time::SystemTime;

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

/// Return `true` when `path` is a directory (following symlinks).
///
/// Mirrors `Path::is_dir`: an unreadable or absent path reports `false` rather
/// than surfacing the metadata error.
///
/// # Examples
///
/// ```
/// let dir = tempfile::tempdir().expect("create tempdir");
/// let file = dir.path().join("file");
/// test_support::fs::write(&file, "contents").expect("write file");
/// assert!(test_support::fs::is_dir(dir.path()));
/// assert!(!test_support::fs::is_dir(&file));
/// assert!(!test_support::fs::is_dir(dir.path().join("absent")));
/// ```
#[must_use]
pub fn is_dir(path: impl AsRef<Path>) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_dir())
}

/// Copy `from` to `to`, returning the number of bytes copied.
///
/// # Errors
///
/// Propagates the underlying `std::fs::copy` failure.
///
/// # Examples
///
/// ```
/// let dir = tempfile::tempdir().expect("create tempdir");
/// let from = dir.path().join("source.txt");
/// let to = dir.path().join("dest.txt");
/// test_support::fs::write(&from, "hello").expect("write source");
/// let bytes = test_support::fs::copy(&from, &to).expect("copy file");
/// assert_eq!(bytes, 5);
/// assert_eq!(test_support::fs::read(&to).expect("read dest"), b"hello");
/// ```
pub fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<u64> {
    fs::copy(from, to)
}

/// Return the modification time of the file at `path`.
///
/// # Errors
///
/// Propagates the underlying metadata failure, or the platform's failure to
/// report a modification time.
///
/// # Examples
///
/// ```
/// use std::time::{Duration, SystemTime};
///
/// let dir = tempfile::tempdir().expect("create tempdir");
/// let path = dir.path().join("fixture.txt");
/// // A second of slack: some filesystems truncate timestamps to whole
/// // seconds, which would put the recorded mtime just behind `before`.
/// let before = SystemTime::now() - Duration::from_secs(1);
/// test_support::fs::write(&path, "hello").expect("write fixture");
/// let mtime = test_support::fs::modified(&path).expect("read mtime");
/// assert!(mtime >= before);
/// ```
pub fn modified(path: impl AsRef<Path>) -> io::Result<SystemTime> {
    fs::metadata(path)?.modified()
}

/// Write `contents` to `path` and set its modification time to `mtime`.
///
/// Backdating a fixture needs the same open file for the write and the
/// timestamp, so both happen here rather than through separate calls. The
/// handle never leaves this function: returning it would push the ambient
/// operation out to the caller, which is what this module exists to prevent.
///
/// # Errors
///
/// Propagates the create, write, or timestamp failure.
///
/// # Examples
///
/// ```
/// # #[cfg(unix)]
/// # {
/// use std::time::{Duration, UNIX_EPOCH};
///
/// let dir = tempfile::tempdir().expect("create tempdir");
/// let path = dir.path().join("fixture.txt");
/// let mtime = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
/// test_support::fs::write_with_mtime(&path, "hello", mtime).expect("write with mtime");
/// assert_eq!(test_support::fs::modified(&path).expect("read mtime"), mtime);
/// assert_eq!(test_support::fs::read(&path).expect("read contents"), b"hello");
/// # }
/// ```
#[cfg(unix)]
pub fn write_with_mtime(
    path: impl AsRef<Path>,
    contents: impl AsRef<[u8]>,
    mtime: SystemTime,
) -> io::Result<()> {
    use std::os::unix::fs::FileExt;
    let file = fs::File::create(path)?;
    file.write_all_at(contents.as_ref(), 0)?;
    file.set_modified(mtime)
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

/// Return `true` when `path` is a regular file with any execute bit set.
///
/// The inverse of [`set_mode`], for probing a sandbox `PATH` the way an
/// executable lookup would. An unreadable or absent path reports `false`.
///
/// # Examples
///
/// ```
/// let dir = tempfile::tempdir().expect("create tempdir");
/// let path = dir.path().join("tool");
/// test_support::fs::write(&path, "#!/bin/sh\n").expect("write stub");
/// assert!(!test_support::fs::is_executable_file(&path));
/// test_support::fs::set_mode(&path, 0o755).expect("mark executable");
/// assert!(test_support::fs::is_executable_file(&path));
/// assert!(!test_support::fs::is_executable_file(dir.path()));
/// ```
#[cfg(unix)]
#[must_use]
pub fn is_executable_file(path: impl AsRef<Path>) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

/// Create a symbolic link at `link` pointing to `target` on Unix.
///
/// # Errors
///
/// Propagates the underlying symbolic-link creation failure.
///
/// # Examples
///
/// ```
/// let dir = tempfile::tempdir().expect("create tempdir");
/// let target = dir.path().join("target");
/// test_support::fs::create_dir(&target).expect("create target");
/// test_support::fs::symlink(&target, dir.path().join("link")).expect("create symlink");
/// ```
#[cfg(unix)]
pub fn symlink(target: impl AsRef<Path>, link: impl AsRef<Path>) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(test)]
mod tests {
    //! Coverage for directory creation at existing-path boundaries.

    use super::{create_dir_all, write};
    use std::io;

    #[test]
    fn create_dir_all_accepts_an_existing_directory() -> io::Result<()> {
        let temp = tempfile::tempdir()?;
        create_dir_all(temp.path())
    }

    #[test]
    fn create_dir_all_rejects_an_existing_file() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let file = temp.path().join("not-a-directory");
        write(&file, b"fixture")?;

        let error = create_dir_all(&file).expect_err("an existing file is not a directory");
        anyhow::ensure!(
            matches!(
                error.kind(),
                io::ErrorKind::AlreadyExists | io::ErrorKind::NotADirectory
            ),
            "existing file should report a directory conflict, got {error:?}"
        );
        Ok(())
    }

    #[test]
    fn create_dir_all_creates_missing_parent_directories() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let nested = temp.path().join("one").join("two");

        create_dir_all(&nested)?;

        anyhow::ensure!(nested.is_dir(), "recursive creation should produce a directory");
        Ok(())
    }
}
