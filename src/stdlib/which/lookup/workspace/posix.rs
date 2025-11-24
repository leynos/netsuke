//! POSIX workspace traversal for the `which` fallback.

use camino::Utf8PathBuf;
use minijinja::{Error, ErrorKind};
use walkdir::WalkDir;

use super::super::is_executable;
use super::{WorkspaceSearchParams, WORKSPACE_MAX_DEPTH, should_visit_entry};
use crate::stdlib::which::env::EnvSnapshot;

pub(super) fn search_workspace(
    env: &EnvSnapshot,
    command: &str,
    params: WorkspaceSearchParams<'_>,
) -> Result<Vec<Utf8PathBuf>, Error> {
    let walker = WalkDir::new(&env.cwd)
        .follow_links(false)
        .max_depth(WORKSPACE_MAX_DEPTH)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| should_visit_entry(entry, params.skip_dirs));

    let matches = collect_matching_executables(walker, command, collect_all)?;

    log_if_empty(&matches, command);

    Ok(matches)
}

/// Convert a `WalkDir` result into an entry, logging and skipping unreadable
/// paths to keep workspace traversal resilient.
fn unwrap_or_log_error(
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

/// Emit a debug log when the workspace traversal yields no executables.
fn log_if_empty(matches: &[Utf8PathBuf], command: &str) {
    if matches.is_empty() {
        tracing::debug!(
            %command,
            max_depth = WORKSPACE_MAX_DEPTH,
            skip = ?params.skip_dirs,
            "workspace which fallback found no matches",
        );
    }
}

/// Traverse the workspace iterator, collecting executable matches and stopping
/// early when `collect_all` is `false` and a match is discovered.
fn collect_matching_executables(
    entries: impl Iterator<Item = Result<walkdir::DirEntry, walkdir::Error>>,
    command: &str,
    collect_all: bool,
) -> Result<Vec<Utf8PathBuf>, Error> {
    let mut matches = Vec::new();

    for walk_entry in entries {
        let Some(entry) = unwrap_or_log_error(walk_entry, command) else {
            continue;
        };

        if let Some(path) = process_workspace_entry(entry, command)? {
            matches.push(path);
            if !params.collect_all {
                break;
            }
        }
    }

    Ok(matches)
}

fn process_workspace_entry(
    entry: walkdir::DirEntry,
    command: &str,
) -> Result<Option<Utf8PathBuf>, Error> {
    if !entry.file_type().is_file() {
        return Ok(None);
    }

    let file_name = entry.file_name().to_string_lossy();
    if file_name != command {
        return Ok(None);
    }

    let path = entry.into_path();
    let utf8 = Utf8PathBuf::from_path_buf(path).map_err(|path_buf| {
        let lossy_path = path_buf.to_string_lossy();
        Error::new(
            ErrorKind::InvalidOperation,
            format!(
                "workspace path contains non-UTF-8 components while resolving command '{command}': {lossy_path}",
            ),
        )
    })?;

    Ok(is_executable(&utf8).then_some(utf8))
}
