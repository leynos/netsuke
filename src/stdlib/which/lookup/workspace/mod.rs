//! Workspace fallback search helpers for the `which` resolver.

use std::hash::{Hash, Hasher};

use camino::Utf8PathBuf;
use indexmap::IndexSet;

use crate::stdlib::which::{env::EnvSnapshot, resolve_error::ResolveError};

#[cfg(not(windows))]
mod posix;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
use posix::search_workspace as platform_search_workspace;
#[cfg(windows)]
use windows::search_workspace as platform_search_workspace;

/// Maximum directory depth the workspace fallback search descends to.
pub(super) const WORKSPACE_MAX_DEPTH: usize = 6;
/// Directory basenames skipped by default during the workspace fallback search.
pub(crate) const WORKSPACE_SKIP_DIRS: &[&str] =
    &[".git", "target", "node_modules", "dist", "build"];

/// Set of directory basenames excluded from the workspace fallback search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkspaceSkipList {
    /// The excluded basenames.
    dirs: IndexSet<String>,
}

impl WorkspaceSkipList {
    /// Build a skip list from the default set of directory names.
    fn from_defaults() -> Self {
        let mut dirs = IndexSet::new();
        for dir in WORKSPACE_SKIP_DIRS {
            dirs.insert((*dir).to_owned());
        }
        Self { dirs }
    }

    /// Whether a directory basename is excluded.
    fn contains(&self, name: &str) -> bool {
        self.dirs.contains(name)
    }

    /// Build a skip list from provided directory basenames, normalising and
    /// de-duplicating entries.
    pub(crate) fn from_names(names: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        let mut dirs = IndexSet::new();
        for name in names {
            let trimmed = name.as_ref().trim();
            if trimmed.is_empty() {
                continue;
            }
            dirs.insert(normalise_name(trimmed));
        }
        Self { dirs }
    }
}

impl Hash for WorkspaceSkipList {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut dirs: Vec<&String> = self.dirs.iter().collect();
        dirs.sort_unstable();
        for dir in dirs {
            dir.hash(state);
        }
    }
}

impl Default for WorkspaceSkipList {
    fn default() -> Self {
        Self::from_defaults()
    }
}

/// Search the working directory tree for `command`, honouring the fallback switch.
pub(super) fn search_workspace(
    env: &EnvSnapshot,
    command: &str,
    collect_all: bool,
    skip_dirs: &WorkspaceSkipList,
) -> Result<Vec<Utf8PathBuf>, ResolveError> {
    if !env.workspace_fallback_enabled() {
        tracing::debug!(
            env = crate::stdlib::which::workspace_switch::WORKSPACE_FALLBACK_ENV,
            "workspace which fallback disabled via env override",
        );
        return Ok(Vec::new());
    }

    tracing::debug!(
        max_depth = WORKSPACE_MAX_DEPTH,
        skip = ?skip_dirs,
        "using workspace which fallback",
    );

    platform_search_workspace(env, command, collect_all, skip_dirs)
}

/// Whether a walkdir entry should be visited, skipping listed directories.
pub(super) fn should_visit_entry(entry: &walkdir::DirEntry, skip_dirs: &WorkspaceSkipList) -> bool {
    if !entry.file_type().is_dir() {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    !skip_dirs.contains(&name)
}

/// Convert a walkdir item to an entry, propagating traversal errors as
/// [`ResolveError::WalkDir`] so callers can surface IO failures rather than
/// silently skipping them.
pub(super) fn unwrap_or_log_error(
    walk_entry: Result<walkdir::DirEntry, walkdir::Error>,
) -> Result<walkdir::DirEntry, ResolveError> {
    walk_entry.map_err(|err| {
        tracing::debug!(
            error = %err,
            "unreadable workspace entry during which fallback",
        );
        ResolveError::WalkDir { source: err }
    })
}

/// Emit a debug message when fallback traversal yields no matches, helping
/// callers diagnose unexpected latency or misses.
pub(super) fn log_if_no_matches(matches: &[Utf8PathBuf], skip_dirs: &WorkspaceSkipList) {
    if matches.is_empty() {
        tracing::debug!(
            max_depth = WORKSPACE_MAX_DEPTH,
            skip = ?skip_dirs,
            "workspace which fallback found no matches",
        );
    }
}

/// Normalise a directory basename for skip matching on Windows.
#[cfg(windows)]
fn normalise_name(name: &str) -> String {
    name.to_ascii_lowercase()
}

/// Normalise a directory basename for skip matching on POSIX.
#[cfg(not(windows))]
fn normalise_name(name: &str) -> String {
    name.to_owned()
}
