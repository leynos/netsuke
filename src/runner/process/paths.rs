//! Path resolution helpers for the Ninja runner.
//! Canonicalizes UTF-8 paths via capability-based handles.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs as cap_fs};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

/// Canonicalize `path`, requiring and returning UTF-8 throughout.
///
/// Empty and `.` paths resolve to the current directory. Absolute-path
/// resolution is anchored by opening the direct parent directory.
///
/// # Errors
///
/// Returns an error when canonicalization fails.
pub fn canonicalize_utf8_path(path: &Utf8Path) -> io::Result<Utf8PathBuf> {
    if path.as_str().is_empty() || path == Utf8Path::new(".") {
        return canonicalize_current_dir();
    }

    if path.parent().is_none() && path.file_name().is_none() {
        return Ok(canonicalize_root_path(path));
    }

    if path.is_relative() {
        return canonicalize_relative_path(path);
    }

    canonicalize_absolute_path(path)
}

/// Canonicalize the current working directory to a UTF-8 path.
///
/// # Errors
///
/// Returns an error when the directory cannot be opened, canonicalisation
/// fails, or the result is not valid UTF-8.
fn canonicalize_current_dir() -> io::Result<Utf8PathBuf> {
    let dir = cap_fs::Dir::open_ambient_dir(".", ambient_authority())?;
    let resolved = dir.canonicalize(Path::new("."))?;
    convert_path_to_utf8(resolved, Utf8Path::new("."))
}

/// Return the filesystem root unchanged, as it is already canonical.
fn canonicalize_root_path(utf8: &Utf8Path) -> Utf8PathBuf {
    utf8.to_path_buf()
}

/// Canonicalize a relative UTF-8 path against the current directory.
///
/// # Errors
///
/// Returns an error when the current directory cannot be opened,
/// canonicalisation fails, or the result is not valid UTF-8.
fn canonicalize_relative_path(utf8: &Utf8Path) -> io::Result<Utf8PathBuf> {
    let dir = cap_fs::Dir::open_ambient_dir(".", ambient_authority())?;
    let resolved = dir.canonicalize(utf8.as_std_path())?;
    convert_path_to_utf8(resolved, utf8)
}

/// Canonicalize an absolute UTF-8 path through its parent directory handle.
///
/// # Errors
///
/// Returns an error when the parent directory cannot be opened,
/// canonicalisation fails, or the result is not valid UTF-8.
fn canonicalize_absolute_path(utf8: &Utf8Path) -> io::Result<Utf8PathBuf> {
    let parent = utf8.parent().unwrap_or_else(|| Utf8Path::new("/"));
    let handle = cap_fs::Dir::open_ambient_dir(parent.as_std_path(), ambient_authority())?;
    let relative = utf8.strip_prefix(parent).unwrap_or(utf8);
    let resolved = handle.canonicalize(relative.as_std_path())?;
    let canonical = convert_path_to_utf8(resolved, relative)?;
    if canonical.is_absolute() {
        Ok(canonical)
    } else {
        let mut absolute = parent.to_path_buf();
        absolute.push(&canonical);
        Ok(absolute)
    }
}

/// Convert a canonical `buf` back to UTF-8, naming the failing reference path.
///
/// # Errors
///
/// Returns an error when the canonical path is not valid UTF-8.
fn convert_path_to_utf8(buf: PathBuf, reference: &Utf8Path) -> io::Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(buf).map_err(|_| {
        io::Error::new(
            ErrorKind::InvalidData,
            format!("canonical path for {reference} is not valid UTF-8"),
        )
    })
}
