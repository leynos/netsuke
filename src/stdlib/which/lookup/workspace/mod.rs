//! Workspace fallback search helpers for the `which` resolver.

use std::{env, hash::Hash};

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
const WORKSPACE_FALLBACK_ENV: &str = "NETSUKE_WHICH_WORKSPACE";

pub(crate) const DEFAULT_WORKSPACE_SKIP_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    ".idea",
    ".vscode",
    "dist",
    "build",
];

/// Normalised set of directory basenames that should be ignored during
/// workspace traversal. Entries are lowercased on Windows to provide
/// case-insensitive membership checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkspaceSkipList {
    dirs: IndexSet<String>,
}

/// Parameters used when scanning the workspace for executables.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WorkspaceSearchParams<'a> {
    pub(crate) collect_all: bool,
    pub(crate) skip_dirs: &'a WorkspaceSkipList,
}

impl WorkspaceSkipList {
    /// Build a skip list from the provided directory names, normalising for the
    /// host platform and removing duplicates.
    ///
    /// Empty or whitespace-only inputs are ignored; callers that require strict
    /// validation should filter earlier (for example via
    /// `StdlibConfig::with_workspace_skip_dirs`).
    pub(crate) fn from_names(names: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        let mut dedup = IndexSet::new();
        for name in names {
            let trimmed = name.as_ref().trim();
            if trimmed.is_empty() {
                continue;
            }
            dedup.insert(normalise_dir_name(trimmed));
        }
        Self { dirs: dedup }
    }

    /// Return `true` when a directory should be skipped during traversal.
    pub(crate) fn should_skip(&self, name: &str) -> bool {
        self.dirs.contains(&normalise_dir_name(name))
    }
}

impl Default for WorkspaceSkipList {
    fn default() -> Self {
        Self::from_names(DEFAULT_WORKSPACE_SKIP_DIRS.iter().copied())
    }
}

impl std::hash::Hash for WorkspaceSkipList {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for dir in &self.dirs {
            dir.hash(state);
        }
    }
}

#[cfg(windows)]
fn normalise_dir_name(name: &str) -> String {
    name.to_ascii_lowercase()
}

#[cfg(not(windows))]
fn normalise_dir_name(name: &str) -> String {
    name.to_owned()
}

pub(super) fn search_workspace(
    env: &EnvSnapshot,
    command: &str,
    params: WorkspaceSearchParams<'_>,
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
        skip = ?params.skip_dirs,
        "using workspace which fallback",
    );

    platform_search_workspace(env, command, params)
}

pub(super) fn should_visit_entry(
    entry: &walkdir::DirEntry,
    skip_dirs: &WorkspaceSkipList,
) -> bool {
    if !entry.file_type().is_dir() {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    !skip_dirs.should_skip(&name)
}

fn workspace_fallback_enabled() -> bool {
    match env::var(WORKSPACE_FALLBACK_ENV) {
        Ok(value) => {
            let normalised = value.to_ascii_lowercase();
            !matches!(normalised.as_str(), "0" | "false" | "off")
        }
        Err(env::VarError::NotPresent | env::VarError::NotUnicode(_)) => true,
    }
}
