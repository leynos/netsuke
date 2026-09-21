//! Typed resolver errors for the `which` stdlib helpers.

use std::{fmt, io};

use camino::{Utf8Path, Utf8PathBuf};
use walkdir;

use super::options::CwdMode;

/// The bounded failure taxonomy of [`ResolveError`].
///
/// The resolver's own vocabulary, kept beside the error type it describes and
/// independent of how any of it is reported. A consumer that needs to name a
/// failure names a variant here; the telemetry layer is one such consumer and
/// spells these for its own label set, so a change to what is *recorded*
/// cannot change what the resolver *means* by a failure.
///
/// The declaration order is the order [`ResolveErrorCategory::ALL_LABELS`]
/// lists, and it is the order the variants appear in [`ResolveError`]. Nothing
/// depends on that beyond readability, but keeping the three in step is what
/// makes the taxonomy legible at a glance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum ResolveErrorCategory {
    /// A PATH search exhausted every candidate directory without a hit.
    NotFound,
    /// A direct-path lookup found no executable at the resolved path.
    DirectNotFound,
    /// An invalid argument or option value was supplied.
    Args,
    /// Canonicalization of a matched path failed.
    Canonicalize,
    /// `fs::metadata` failed whilst checking whether a path is executable.
    IsExecutable,
    /// A canonical path could not be represented as UTF-8.
    CanonicalizeNonUtf8,
    /// A workspace fallback path could not be represented as UTF-8.
    WorkspaceNonUtf8,
    /// A `walkdir` traversal error encountered during workspace fallback search.
    WalkDir,
    /// The working directory could not be read.
    CwdResolve,
    /// The working directory path is not valid UTF-8.
    CwdNonUtf8,
}

impl ResolveErrorCategory {
    /// Every category's spelling, in declaration order.
    ///
    /// One entry per variant, each spelled by [`ResolveErrorCategory::label`],
    /// so this list and the per-variant spellings cannot disagree about a
    /// *word*: the words are written once, in `label`, and this array calls it.
    /// It exists so a consumer that needs the whole vocabulary — the telemetry
    /// boundary, and the tests that hold the label set to it — has one place to
    /// read it from.
    ///
    /// The *set*, though, is a second declaration, and that half is not
    /// self-maintaining. A variant added with a `label` arm and a `category`
    /// arm still compiles while this array holds ten entries, because the
    /// length is part of the type. What stops the omission is the exhaustive
    /// `match` in `label`, in `category`, and in the boundary's
    /// `category_label`: the compiler refuses each until the new variant is
    /// named, and naming it in `label` is what puts its word in reach here.
    /// Deciding that it belongs in this list is then a deliberate act, held in
    /// place by the tests rather than by the type.
    pub(super) const ALL_LABELS: [&'static str; 10] = [
        Self::NotFound.label(),
        Self::DirectNotFound.label(),
        Self::Args.label(),
        Self::Canonicalize.label(),
        Self::IsExecutable.label(),
        Self::CanonicalizeNonUtf8.label(),
        Self::WorkspaceNonUtf8.label(),
        Self::WalkDir.label(),
        Self::CwdResolve.label(),
        Self::CwdNonUtf8.label(),
    ];

    /// The lowercase spelling the domain uses for this category.
    ///
    /// This is the resolver's own name for the failure, and the string a
    /// diagnostic carries when it names a category rather than an instance.
    /// It is deliberately the same spelling the telemetry label uses: the
    /// label is this taxonomy exported, not a parallel naming of it, and the
    /// telemetry boundary is where the two are tied together.
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::DirectNotFound => "direct_not_found",
            Self::Args => "args",
            Self::Canonicalize => "canonicalize",
            Self::IsExecutable => "is_executable",
            Self::CanonicalizeNonUtf8 => "canonicalize_non_utf8",
            Self::WorkspaceNonUtf8 => "workspace_non_utf8",
            Self::WalkDir => "walkdir",
            Self::CwdResolve => "cwd_resolve",
            Self::CwdNonUtf8 => "cwd_non_utf8",
        }
    }
}

/// Typed errors raised while resolving a command with `which`.
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
    /// Canonicalization of a matched path failed.
    Canonicalize {
        /// The path that failed to canonicalize.
        path: Utf8PathBuf,
        /// The underlying filesystem error.
        source: io::Error,
    },
    /// `fs::metadata` failed whilst checking whether a path is executable.
    IsExecutable {
        /// The path whose executable bit could not be probed.
        path: Utf8PathBuf,
        /// The metadata error that prevented the probe.
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
    WalkDir {
        /// The traversal error from the workspace fallback walk.
        source: walkdir::Error,
    },
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

    /// Return the domain category this error belongs to.
    ///
    /// The return type is the resolver's own [`ResolveErrorCategory`] rather
    /// than a string, so the pairing of variant to category is a fact this
    /// module states and the type system checks: an arm cannot return a value
    /// outside the taxonomy, and no consumer can read the taxonomy without
    /// naming the type it belongs to. Reporting the category — as a label, a
    /// log field, or a diagnostic — is a separate decision made by whoever
    /// reports it.
    ///
    /// Every variant is listed explicitly rather than caught by a wildcard, so
    /// adding a variant is a compile error here until its category is decided.
    pub(super) const fn category(&self) -> ResolveErrorCategory {
        match self {
            Self::NotFound { .. } => ResolveErrorCategory::NotFound,
            Self::DirectNotFound { .. } => ResolveErrorCategory::DirectNotFound,
            Self::Args { .. } => ResolveErrorCategory::Args,
            Self::Canonicalize { .. } => ResolveErrorCategory::Canonicalize,
            Self::IsExecutable { .. } => ResolveErrorCategory::IsExecutable,
            Self::CanonicalizeNonUtf8 => ResolveErrorCategory::CanonicalizeNonUtf8,
            Self::WorkspaceNonUtf8 { .. } => ResolveErrorCategory::WorkspaceNonUtf8,
            Self::WalkDir { .. } => ResolveErrorCategory::WalkDir,
            Self::CwdResolve { .. } => ResolveErrorCategory::CwdResolve,
            Self::CwdNonUtf8 => ResolveErrorCategory::CwdNonUtf8,
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
            _ => formatter.write_str(self.category().label()),
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
