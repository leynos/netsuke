//! Workspace fallback search helpers for the `which` resolver.

use std::{
    env,
    hash::{Hash, Hasher},
};

use camino::Utf8PathBuf;
use indexmap::IndexSet;
use minijinja::Error;

use crate::stdlib::which::env::EnvSnapshot;

#[cfg(not(windows))]
mod posix;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
use posix::search_workspace as platform_search_workspace;
#[cfg(windows)]
use windows::search_workspace as platform_search_workspace;

pub(super) const WORKSPACE_MAX_DEPTH: usize = 6;
pub(crate) const WORKSPACE_SKIP_DIRS: &[&str] =
    &[".git", "target", "node_modules", "dist", "build"];

const WORKSPACE_FALLBACK_ENV: &str = "NETSUKE_WHICH_WORKSPACE";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkspaceSkipList {
    dirs: IndexSet<String>,
}

impl WorkspaceSkipList {
    fn from_defaults() -> Self {
        let mut dirs = IndexSet::new();
        for dir in WORKSPACE_SKIP_DIRS {
            dirs.insert((*dir).to_owned());
        }
        Self { dirs }
    }

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

pub(super) fn search_workspace(
    env: &EnvSnapshot,
    command: &str,
    collect_all: bool,
    skip_dirs: &WorkspaceSkipList,
) -> Result<Vec<Utf8PathBuf>, Error> {
    if !workspace_fallback_enabled() {
        tracing::debug!(
            %command,
            env = WORKSPACE_FALLBACK_ENV,
            "workspace which fallback disabled via env override",
        );
        return Ok(Vec::new());
    }

    tracing::debug!(
        %command,
        max_depth = WORKSPACE_MAX_DEPTH,
        skip = ?skip_dirs,
        "using workspace which fallback",
    );

    platform_search_workspace(env, command, collect_all, skip_dirs)
}

pub(super) fn should_visit_entry(entry: &walkdir::DirEntry, skip_dirs: &WorkspaceSkipList) -> bool {
    if !entry.file_type().is_dir() {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    !skip_dirs.contains(&name)
}

fn workspace_fallback_enabled() -> bool {
    match env::var(WORKSPACE_FALLBACK_ENV) {
        Ok(value) => {
            let normalised = value.to_ascii_lowercase();
            !matches!(normalised.as_str(), "0" | "false" | "off")
        }
        Err(env::VarError::NotPresent) => true,
        Err(env::VarError::NotUnicode(_)) => {
            tracing::warn!(
                env = WORKSPACE_FALLBACK_ENV,
                "workspace fallback disabled because env var is not valid UTF-8",
            );
            false
        }
    }
}

/// Convert a walkdir item to an entry, logging and skipping unreadable paths to
/// keep workspace traversal robust across platforms.
pub(super) fn unwrap_or_log_error(
    walk_entry: Result<walkdir::DirEntry, walkdir::Error>,
    command: &str,
) -> Option<walkdir::DirEntry> {
    match walk_entry {
        Ok(entry) => Some(entry),
        Err(err) => {
            tracing::debug!(
                %command,
                error = %err,
                "skipping unreadable workspace entry during which fallback",
            );
            None
        }
    }
}

/// Emit a debug message when fallback traversal yields no matches, helping
/// callers diagnose unexpected latency or misses.
pub(super) fn log_if_no_matches(
    matches: &[Utf8PathBuf],
    command: &str,
    skip_dirs: &WorkspaceSkipList,
) {
    if matches.is_empty() {
        tracing::debug!(
            %command,
            max_depth = WORKSPACE_MAX_DEPTH,
            skip = ?skip_dirs,
            "workspace which fallback found no matches",
        );
    }
}

#[cfg(windows)]
fn normalise_name(name: &str) -> String {
    name.to_ascii_lowercase()
}

#[cfg(not(windows))]
fn normalise_name(name: &str) -> String {
    name.to_owned()
}
