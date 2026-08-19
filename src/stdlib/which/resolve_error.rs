//! Typed resolver errors for the `which` stdlib helpers.

use std::{fmt, io};

use camino::{Utf8Path, Utf8PathBuf};
use walkdir;

use super::options::CwdMode;

/// Typed errors raized while resolving a command with `which`.
#[derive(Debug)]
pub(crate) enum ResolveError {
    /// A PATH search exhausted every candidate directory without a hit.
    NotFound {
        /// The command that was searched for.
        command: String,
        /// The directories searched, kept for the diagnostic preview.
        dirs: Vec<Utf8PathBuf>,
        /// How the current directory contributed to the search.
        cwd_mode: CwdMode,
    },
    /// A direct-path lookup found no executable at the resolved path.
    DirectNotFound {
        /// The path-like command that was looked up.
        command: String,
        /// The resolved candidate that was not executable.
        path: Utf8PathBuf,
    },
    /// An invalid argument or option value was supplied.
    Args {
        /// Human-readable explanation of the invalid input.
        detail: String,
    },
    /// Canonicalisation of a matched path failed.
    Canonicalize {
        /// The path that failed to canonicalize.
        path: Utf8PathBuf,
        /// The underlying filesystem error.
        source: io::Error,
    },
    /// `fs::metadata` failed whilst checking whether a path is executable.
    IsExecutable {
        path: Utf8PathBuf,
        source: io::Error,
    },
    /// A canonical path could not be represented as UTF-8.
    CanonicalizeNonUtf8,
    /// A workspace fallback path could not be represented as UTF-8.
    WorkspaceNonUtf8 {
        /// The command being searched for.
        command: String,
        /// The non-UTF-8 path, rendered lossily for the diagnostic.
        path: String,
    },
    /// A `walkdir` traversal error encountered during workspace fallback search.
    WalkDir { source: walkdir::Error },
    /// The working directory could not be read.
    CwdResolve {
        /// The underlying filesystem error.
        source: io::Error,
    },
    /// The working directory path is not valid UTF-8.
    CwdNonUtf8,
}

impl ResolveError {
    /// Construct an argument error for invalid or unexpected resolver options.
    ///
    /// `detail` is a human-readable explanation included in the localized
    /// diagnostic.
    pub(super) fn args(detail: impl fmt::Display) -> Self {
        Self::Args {
            detail: detail.to_string(),
        }
    }

    /// Return the stable low-cardinality category used by logs and metrics.
    pub(super) const fn category(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "not_found",
            Self::DirectNotFound { .. } => "direct_not_found",
            Self::Args { .. } => "args",
            Self::Canonicalize { .. } => "canonicalize",
            Self::IsExecutable { .. } => "is_executable",
            Self::CanonicalizeNonUtf8 => "canonicalize_non_utf8",
            Self::WorkspaceNonUtf8 { .. } => "workspace_non_utf8",
            Self::WalkDir { .. } => "walkdir",
            Self::CwdResolve { .. } => "cwd_resolve",
            Self::CwdNonUtf8 => "cwd_non_utf8",
        }
    }
}

impl fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IsExecutable { path, source } => {
                write!(
                    formatter,
                    "failed to inspect executable path '{path}': {source}"
                )
            }
            _ => formatter.write_str(self.category()),
        }
    }
}

impl std::error::Error for ResolveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Canonicalize { source, .. }
            | Self::IsExecutable { source, .. }
            | Self::CwdResolve { source } => Some(source),
            Self::WalkDir { source } => Some(source),
            Self::NotFound { .. }
            | Self::DirectNotFound { .. }
            | Self::Args { .. }
            | Self::CanonicalizeNonUtf8
            | Self::WorkspaceNonUtf8 { .. }
            | Self::CwdNonUtf8 => None,
        }
    }
}

/// Construct a PATH-search miss after all candidate directories are exhausted.
///
/// `command` is the lookup key, `dirs` is the searched directory set, and
/// `mode` records how the current directory contributed to the search.
pub(super) fn not_found(command: &str, dirs: &[&Utf8Path], mode: CwdMode) -> ResolveError {
    ResolveError::NotFound {
        command: command.to_owned(),
        dirs: dirs.iter().map(|dir| dir.to_path_buf()).collect(),
        cwd_mode: mode,
    }
}

/// Construct a direct-path miss for a path-like command lookup.
///
/// Use this when the command names a filesystem path rather than a PATH search;
/// `path` is the resolved candidate that was not executable.
pub(super) fn direct_not_found_error(command: &str, path: &camino::Utf8Path) -> ResolveError {
    ResolveError::DirectNotFound {
        command: command.to_owned(),
        path: path.to_path_buf(),
    }
}
