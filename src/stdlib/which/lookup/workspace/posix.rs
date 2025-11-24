//! POSIX workspace traversal for the `which` fallback.

use camino::Utf8PathBuf;
use minijinja::{Error, ErrorKind};
use walkdir::WalkDir;

use super::super::is_executable;
use super::{log_if_no_matches, should_visit_entry, unwrap_or_log_error, WORKSPACE_MAX_DEPTH};
use crate::stdlib::which::env::EnvSnapshot;

pub(super) fn search_workspace(
    env: &EnvSnapshot,
    command: &str,
    collect_all: bool,
) -> Result<Vec<Utf8PathBuf>, Error> {
    let walker = WalkDir::new(&env.cwd)
        .follow_links(false)
        .max_depth(WORKSPACE_MAX_DEPTH)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(should_visit_entry);

    let matches = collect_matching_executables(walker, command, collect_all)?;
    log_if_no_matches(&matches, command);
    Ok(matches)
}

fn collect_matching_executables(
    walker: impl Iterator<Item = Result<walkdir::DirEntry, walkdir::Error>>,
    command: &str,
    collect_all: bool,
) -> Result<Vec<Utf8PathBuf>, Error> {
    let mut matches = Vec::new();

    for walk_entry in walker {
        let Some(entry) = unwrap_or_log_error(walk_entry, command) else {
            continue;
        };

        if let Some(path) = process_workspace_entry(entry, command)? {
            matches.push(path);
            if !collect_all {
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
