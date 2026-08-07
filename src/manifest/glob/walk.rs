//! Filesystem traversal helpers for glob expansion.
//!
//! Glob matching itself is performed by the `glob` crate, which walks the
//! filesystem ambiently. The metadata checks used to filter directories out
//! of the results, however, go through a capability-scoped
//! [`cap_std::fs::Dir`] handle. To honour least privilege, that handle is
//! opened at the pattern's longest literal directory prefix (for example
//! `src/` for `src/**/*.c`) rather than at the filesystem root, so the
//! capability covers only the subtree the pattern can actually match. A
//! symbolic link whose target escapes that subtree is therefore unreadable
//! through the capability; such a match is skipped rather than failing the
//! expansion.

use super::{GlobEntryResult, GlobErrorContext, GlobErrorType, GlobPattern, create_glob_error};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};
use minijinja::Error;
use std::io;

/// Capability root for a glob expansion.
///
/// Couples the [`Dir`] handle opened at the pattern's literal prefix with
/// that prefix, so matched paths can be relativised before metadata lookups.
pub(super) struct GlobRoot {
    dir: Dir,
    prefix: Utf8PathBuf,
}

impl GlobRoot {
    #[cfg(test)]
    pub(super) const fn new(dir: Dir, prefix: Utf8PathBuf) -> Self {
        Self { dir, prefix }
    }

    /// Directory the capability is scoped to.
    #[cfg(test)]
    pub(super) const fn dir(&self) -> &Dir {
        &self.dir
    }

    /// Literal pattern prefix the capability was opened at.
    #[cfg(test)]
    pub(super) fn prefix(&self) -> &Utf8Path {
        self.prefix.as_path()
    }

    /// Fetch metadata for a matched path via the capability-scoped handle.
    ///
    /// Returns `Ok(None)` when the entry is a symbolic link that cannot be
    /// resolved through the capability — because its target lies outside the
    /// literal prefix, or because it dangles. Such an entry names no file
    /// reachable within the capability, so it is skipped rather than aborting
    /// the whole expansion.
    pub(super) fn metadata(&self, path: &Utf8Path) -> io::Result<Option<cap_std::fs::Metadata>> {
        let relative = self.relativise(path)?;
        match self.dir.metadata(relative) {
            Ok(metadata) => Ok(Some(metadata)),
            Err(err) => {
                if self
                    .dir
                    .symlink_metadata(relative)
                    .is_ok_and(|link| link.is_symlink())
                {
                    Ok(None)
                } else {
                    Err(err)
                }
            }
        }
    }

    /// Rebase a matched path onto the capability prefix.
    fn relativise<'a>(&self, path: &'a Utf8Path) -> io::Result<&'a Utf8Path> {
        let relative = if self.prefix == "." {
            path
        } else {
            path.strip_prefix(&self.prefix).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "glob match {path} does not start with capability prefix {}",
                        self.prefix
                    ),
                )
            })?
        };
        Ok(if relative.as_str().is_empty() {
            Utf8Path::new(".")
        } else {
            relative
        })
    }
}

/// Longest literal directory prefix of a normalised pattern.
///
/// Scans up to the first glob metacharacter (`*`, `?`, `[`, `{`) and trims
/// back to the last path separator, yielding the deepest directory that the
/// pattern names literally. Returns `.` when the pattern has no literal
/// directory component.
pub(super) fn literal_dir_prefix(normalized: &str) -> &str {
    let meta_idx = normalized
        .find(['*', '?', '[', '{'])
        .unwrap_or(normalized.len());
    let literal = normalized.get(..meta_idx).unwrap_or_default();
    // Keep the trailing separator so absolute roots stay absolute ("/").
    literal
        .rfind(std::path::MAIN_SEPARATOR)
        .and_then(|idx| literal.get(..=idx))
        .unwrap_or(".")
}

/// Open the directory used as the capability root for the glob.
///
/// Returns `Ok(None)` when the literal prefix does not exist (or is not a
/// directory); the pattern can match nothing in that case, mirroring the
/// empty result the matcher would produce.
pub(super) fn open_root_dir(pattern: &GlobPattern) -> io::Result<Option<GlobRoot>> {
    let prefix = literal_dir_prefix(pattern.normalized());
    match Dir::open_ambient_dir(prefix, ambient_authority()) {
        Ok(dir) => Ok(Some(GlobRoot {
            dir,
            prefix: Utf8PathBuf::from(prefix),
        })),
        Err(err)
            if matches!(
                err.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(None)
        }
        Err(err) => Err(err),
    }
}

fn create_io_error(pattern: &GlobPattern, position: usize, detail: String) -> Error {
    create_glob_error(
        &GlobErrorContext {
            pattern: pattern.raw().to_owned(),
            error_char: '\0',
            position,
            error_type: GlobErrorType::IoError,
        },
        Some(detail),
    )
}

/// Process a single glob entry, normalising UTF-8 paths and filtering
/// directories.
pub(super) fn process_glob_entry(
    entry: GlobEntryResult,
    pattern: &GlobPattern,
    root: &GlobRoot,
) -> std::result::Result<Option<String>, Error> {
    match entry {
        Ok(path) => {
            let utf_path = Utf8PathBuf::try_from(path).map_err(|_| {
                create_io_error(
                    pattern,
                    pattern.raw().len(),
                    "glob matched a non-UTF-8 path".to_owned(),
                )
            })?;
            let metadata = root
                .metadata(&utf_path)
                .map_err(|err| create_io_error(pattern, pattern.raw().len(), err.to_string()))?;
            if !metadata.is_some_and(|found| found.is_file()) {
                return Ok(None);
            }
            Ok(Some(utf_path.as_str().replace('\\', "/")))
        }
        Err(e) => Err(create_io_error(pattern, 0, e.to_string())),
    }
}
