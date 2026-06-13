//! Typed resolver errors for the `which` stdlib helpers.

use std::{fmt, io};

use camino::Utf8PathBuf;

use super::options::CwdMode;

#[derive(Debug)]
pub(crate) enum ResolveError {
    NotFound {
        command: String,
        dirs: Vec<Utf8PathBuf>,
        cwd_mode: CwdMode,
    },
    DirectNotFound {
        command: String,
        path: Utf8PathBuf,
    },
    Args {
        detail: String,
    },
    Canonicalise {
        path: Utf8PathBuf,
        source: io::Error,
    },
    CanonicaliseNonUtf8,
    WorkspaceNonUtf8 {
        command: String,
        path: String,
    },
    CwdResolve {
        source: io::Error,
    },
    CwdNonUtf8,
}

impl ResolveError {
    /// Construct an argument error for invalid or unexpected resolver options.
    ///
    /// `detail` is a human-readable explanation included in the localised
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
            Self::Canonicalise { .. } => "canonicalise",
            Self::CanonicaliseNonUtf8 => "canonicalise_non_utf8",
            Self::WorkspaceNonUtf8 { .. } => "workspace_non_utf8",
            Self::CwdResolve { .. } => "cwd_resolve",
            Self::CwdNonUtf8 => "cwd_non_utf8",
        }
    }
}

impl fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.category())
    }
}

impl std::error::Error for ResolveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Canonicalise { source, .. } | Self::CwdResolve { source } => Some(source),
            Self::NotFound { .. }
            | Self::DirectNotFound { .. }
            | Self::Args { .. }
            | Self::CanonicaliseNonUtf8
            | Self::WorkspaceNonUtf8 { .. }
            | Self::CwdNonUtf8 => None,
        }
    }
}

/// Construct a PATH-search miss after all candidate directories are exhausted.
///
/// `command` is the lookup key, `dirs` is the searched directory set, and
/// `mode` records how the current directory contributed to the search.
pub(super) fn not_found(command: &str, dirs: &[Utf8PathBuf], mode: CwdMode) -> ResolveError {
    ResolveError::NotFound {
        command: command.to_owned(),
        dirs: dirs.to_vec(),
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
