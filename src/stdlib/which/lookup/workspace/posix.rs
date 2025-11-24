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
    let mut matches = Vec::new();

    for walk_entry in WalkDir::new(&env.cwd)
        .follow_links(false)
        .max_depth(WORKSPACE_MAX_DEPTH)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| should_visit_entry(entry, params.skip_dirs))
    {
        let entry = match walk_entry {
            Ok(entry) => entry,
            Err(err) => {
                tracing::debug!(
                    %command,
                    error = %err,
                    "skipping unreadable workspace entry during which fallback",
                );
                continue;
            }
        };

        if let Some(path) = process_workspace_entry(entry, command)? {
            matches.push(path);
            if !params.collect_all {
                break;
            }
        }
    }

    if matches.is_empty() {
        tracing::debug!(
            %command,
            max_depth = WORKSPACE_MAX_DEPTH,
            skip = ?params.skip_dirs,
            "workspace which fallback found no matches",
        );
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
