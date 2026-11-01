//! Classify the project configuration file's identity for authority decisions.
//!
//! Discovery computes the project key once per scan, so each candidate root and
//! each stored layer occurrence can be compared against it without
//! re-normalizing every recorded path. The key separates two questions that the
//! merge must not conflate: whether a path *is* the project file, and whether
//! that identity could be established at all.

use super::super::paths::{PathNormalizer, normalized_path_key};
use camino::Utf8Path;
use std::path::{Path, PathBuf};

/// Normalized comparison key for the project configuration file.
pub(super) struct ProjectKey {
    /// Normalized key, or the literal spelling when resolution failed.
    key: PathBuf,
    /// Literal spelling, retained only when the key is not valid UTF-8.
    lossy: Option<String>,
    /// Whether an existing project file could not be normalized.
    ///
    /// An unresolvable project file cannot be proven distinct from any
    /// occurrence, so discovery withholds operator authority rather than grant
    /// it to a project-controlled file.
    unresolved: bool,
}

impl ProjectKey {
    /// Build the comparison key while keeping resolution failures observable.
    pub(super) fn classify(normalizer: &impl PathNormalizer, path: &Path) -> Self {
        let literal = path.to_string_lossy().into_owned();
        let (key, is_resolved) = normalized_path_key(normalizer, &literal)
            .map_or_else(|_| (PathBuf::from(&literal), false), |key| (key, true));
        Self {
            lossy: key.to_str().is_none().then_some(literal),
            key,
            // A missing project file cannot appear as an occurrence, so literal
            // fallback stays trustworthy; one that exists but will not resolve
            // does not.
            unresolved: !is_resolved && path.exists(),
        }
    }

    /// Report whether a recorded layer path is the project configuration file.
    pub(super) fn matches(&self, path: &Utf8Path) -> bool {
        self.matches_path(path.as_std_path())
    }

    /// Report whether a plain path is literally the project configuration file.
    ///
    /// Identification stays literal even when normalization failed, so the
    /// existing project-scope second pass keeps the same deduplication outcome.
    pub(super) fn matches_path(&self, path: &Path) -> bool {
        self.key.as_os_str() == path.as_os_str()
            || self
                .lossy
                .as_deref()
                .is_some_and(|lossy| lossy == path.to_string_lossy())
    }

    /// Report whether the project file's identity could not be established.
    ///
    /// Authority classification treats every occurrence as possibly the project
    /// file in that state; only identification, not trust, is ambiguous.
    pub(super) const fn is_ambiguous(&self) -> bool {
        self.unresolved
    }
}
