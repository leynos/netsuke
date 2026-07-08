//! Deliberately ambient filesystem probes for executable lookup.
//!
//! Netsuke routes filesystem access through capability-scoped `cap_std`
//! handles, enforced by Whitaker's `no_std_fs_operations` lint. The `which`
//! resolver is the one deliberate exception: it consults directories taken
//! from the ambient `PATH` environment variable and canonicalises candidates
//! across arbitrary symlinks, which capability-scoped handles cannot express
//! (a `cap_std` directory handle refuses to follow a symlink that leaves the
//! directory, breaking common layouts such as `/usr/bin/cc ->
//! /etc/alternatives/cc`).
//!
//! This crate confines those ambient operations to a single leaf crate that
//! is excluded from the lint via `dylint.toml`, mirroring the pattern used by
//! other df12 projects. Scope and reuse policy: only the `which` resolver and
//! runner tempfile plumbing may call into this crate; new ambient access
//! elsewhere should use `cap_std` instead.

use std::{fs, io};

use camino::Utf8Path;

/// Return `true` when `path` names an executable regular file.
///
/// Follows symlinks, matching `std::fs::metadata` semantics. On Unix an
/// executable file needs at least one execute permission bit; on other
/// platforms any regular file qualifies.
///
/// # Errors
///
/// Propagates metadata errors, including `NotFound` when the path is absent;
/// callers decide whether absence is an error.
///
/// # Examples
///
/// ```
/// use camino::Utf8Path;
///
/// let sh = Utf8Path::new("/bin/sh");
/// if let Ok(executable) = ambient_fs::is_executable_file(sh) {
///     assert!(executable, "/bin/sh should be executable where it exists");
/// }
/// ```
pub fn is_executable_file(path: &Utf8Path) -> io::Result<bool> {
    let metadata = fs::metadata(path.as_std_path())?;
    Ok(metadata.is_file() && has_execute_permission(&metadata))
}

#[cfg(unix)]
fn has_execute_permission(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn has_execute_permission(metadata: &fs::Metadata) -> bool {
    metadata.is_file()
}

/// Canonicalise `path`, resolving symlinks across the whole filesystem.
///
/// Returns the canonical path as a raw `std::path::PathBuf` so callers keep
/// control over UTF-8 validation and its error mapping.
///
/// # Errors
///
/// Propagates the underlying `canonicalize` failure (missing path, permission
/// denied, and so on).
///
/// # Examples
///
/// ```
/// use camino::Utf8Path;
///
/// let canonical = ambient_fs::canonicalize(Utf8Path::new(".")).expect("cwd resolves");
/// assert!(canonical.is_absolute());
/// ```
pub fn canonicalize(path: &Utf8Path) -> io::Result<std::path::PathBuf> {
    fs::canonicalize(path.as_std_path())
}

/// Return `true` when a walk entry refers to a regular file.
///
/// # Examples
///
/// ```
/// let mut walker = walkdir::WalkDir::new(".").max_depth(1).into_iter();
/// let root = walker.next().expect("walk yields the root").expect("root is readable");
/// assert!(!ambient_fs::entry_is_file(&root), "the root directory is not a file");
/// ```
#[must_use]
pub fn entry_is_file(entry: &walkdir::DirEntry) -> bool {
    entry.file_type().is_file()
}

/// Return `true` when a walk entry refers to a directory.
///
/// # Examples
///
/// ```
/// let mut walker = walkdir::WalkDir::new(".").max_depth(1).into_iter();
/// let root = walker.next().expect("walk yields the root").expect("root is readable");
/// assert!(ambient_fs::entry_is_dir(&root), "the walk root is a directory");
/// ```
#[must_use]
pub fn entry_is_dir(entry: &walkdir::DirEntry) -> bool {
    entry.file_type().is_dir()
}

/// Flush a file's contents and metadata to disk via its open handle.
///
/// Used for tempfiles that live in the ambient system temp directory, where
/// no capability-scoped directory handle exists to sync through.
///
/// # Errors
///
/// Propagates the underlying `sync_all` failure.
///
/// # Examples
///
/// ```
/// let file = tempfile::tempfile().expect("create tempfile");
/// ambient_fs::sync_file(&file).expect("sync tempfile");
/// ```
pub fn sync_file(file: &fs::File) -> io::Result<()> {
    file.sync_all()
}
