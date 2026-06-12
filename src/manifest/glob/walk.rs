//! Filesystem traversal helpers for glob expansion.
//!
//! Glob matching itself is performed by the `glob` crate, which walks the
//! filesystem ambiently. The metadata checks used to filter directories out
//! of the results, however, go through a capability-scoped
//! [`cap_std::fs::Dir`] handle. To honour least privilege, that handle is
//! opened at the pattern's longest literal directory prefix (for example
//! `src/` for `src/**/*.c`) rather than at the filesystem root, so the
//! capability covers only the subtree the pattern can actually match.

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

    /// Fetch metadata for a matched path via the capability-scoped handle.
    fn metadata(&self, path: &Utf8Path) -> io::Result<cap_std::fs::Metadata> {
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
        if relative.as_str().is_empty() {
            self.dir.metadata(Utf8Path::new("."))
        } else {
            self.dir.metadata(relative)
        }
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
            if !metadata.is_file() {
                return Ok(None);
            }
            Ok(Some(utf_path.as_str().replace('\\', "/")))
        }
        Err(e) => Err(create_io_error(pattern, 0, e.to_string())),
    }
}
