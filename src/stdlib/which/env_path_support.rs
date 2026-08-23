//! Parse path-related environment inputs for the `which` resolver.
//!
//! This private support module is owned exclusively by `which::env`. It keeps
//! platform-specific `PATH` and `PATHEXT` parsing below Whitaker's 400-line
//! cap without widening the resolver's public surface. Only `which::env` may
//! import these helpers; sibling lookup modules continue to use its
//! `pub(super)` re-exports.

use std::ffi::OsStr;

use camino::{Utf8Path, Utf8PathBuf};
#[cfg(any(windows, test))]
use indexmap::IndexSet;

use crate::localization::{self, keys};

use super::super::resolve_error::ResolveError;

/// Represent one parsed `PATH` component in search order.
#[derive(Clone, Debug)]
pub(super) enum PathEntry {
    /// Store a directory resolved against the working directory when relative.
    Dir(Utf8PathBuf),
    /// Record an empty component that searches the current working directory.
    CurrentDir,
}

/// Parse a raw `PATH` value into directory entries.
///
/// # Errors
///
/// Returns a [`ResolveError`] when a path component is not valid UTF-8.
pub(super) fn parse_path_entries(
    raw: Option<&OsStr>,
    cwd: &Utf8Path,
) -> Result<Vec<PathEntry>, ResolveError> {
    let mut entries = Vec::new();
    let Some(raw_value) = raw else {
        return Ok(entries);
    };
    for (index, component) in std::env::split_paths(raw_value).enumerate() {
        if component.as_os_str().is_empty() {
            entries.push(PathEntry::CurrentDir);
            continue;
        }
        let utf8 = Utf8PathBuf::from_path_buf(component).map_err(|_| {
            ResolveError::args(
                localization::message(keys::STDLIB_WHICH_PATH_ENTRY_NON_UTF8)
                    .with_arg("index", index),
            )
        })?;
        let resolved = if utf8.is_absolute() {
            utf8
        } else {
            cwd.join(utf8)
        };
        entries.push(PathEntry::Dir(resolved));
    }
    Ok(entries)
}

/// List extensions Windows treats as executable when `PATHEXT` is unset or empty.
///
/// Compiled on Windows and under test alongside [`parse_pathext`], which falls
/// back to this list.
#[cfg(any(windows, test))]
pub(in crate::stdlib::which) const DEFAULT_PATHEXT: &[&str] = &[
    ".com", ".exe", ".bat", ".cmd", ".vbs", ".vbe", ".js", ".jse", ".wsf", ".wsh", ".msc",
];

/// Construct the built-in `PATHEXT` fallback list.
///
/// Entries are already lowercase and dot-prefixed, so they need no further
/// normalization.
#[cfg(any(windows, test))]
fn default_pathext() -> Vec<String> {
    DEFAULT_PATHEXT.iter().copied().map(String::from).collect()
}

/// Normalize a raw `PATHEXT` value into lowercase, dot-prefixed extensions.
///
/// Pure string handling is compiled on Windows and under test so Unix CI can
/// verify lowercasing, leading-dot insertion, trimming, de-duplication, and
/// fallback behaviour.
///
/// # Examples
///
/// ```rust,ignore
/// assert_eq!(parse_pathext(Some(OsStr::new("COM;.com"))), vec![".com"]);
/// ```
#[cfg(any(windows, test))]
pub(in crate::stdlib::which) fn parse_pathext(raw: Option<&OsStr>) -> Vec<String> {
    let mut dedup = IndexSet::new();
    let source = raw.map_or_else(
        || DEFAULT_PATHEXT.join(";"),
        |value| value.to_string_lossy().into_owned(),
    );
    for segment in source.split(';') {
        let trimmed = segment.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut normalised = trimmed.to_ascii_lowercase();
        if !normalised.starts_with('.') {
            normalised.insert(0, '.');
        }
        dedup.insert(normalised);
    }
    if dedup.is_empty() {
        default_pathext()
    } else {
        dedup.into_iter().collect()
    }
}

/// Read the current directory as a UTF-8 path.
///
/// # Errors
///
/// Returns a [`ResolveError`] when the current directory cannot be read or is
/// not valid UTF-8.
pub(super) fn current_dir_utf8() -> Result<Utf8PathBuf, ResolveError> {
    let cwd = std::env::current_dir().map_err(|source| ResolveError::CwdResolve { source })?;
    Utf8PathBuf::from_path_buf(cwd).map_err(|_| ResolveError::CwdNonUtf8)
}

/// Build the Windows candidate paths for `command` within one directory.
#[cfg(windows)]
pub(super) fn candidate_paths(
    dir: &Utf8Path,
    command: &str,
    pathext: &[String],
) -> Vec<Utf8PathBuf> {
    let mut paths = Vec::new();
    let base = dir.join(command);
    if Utf8Path::new(command).extension().is_some() {
        paths.push(base);
        return paths;
    }
    for ext in pathext {
        let mut candidate = base.as_str().to_owned();
        candidate.push_str(ext);
        paths.push(Utf8PathBuf::from(candidate));
    }
    paths
}
