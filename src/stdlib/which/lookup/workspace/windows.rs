//! Windows workspace traversal for the `which` fallback.

use std::collections::HashSet;

use camino::{Utf8Path, Utf8PathBuf};
use minijinja::{Error, ErrorKind};
use walkdir::WalkDir;

use super::super::is_executable;
use super::{
    WORKSPACE_MAX_DEPTH, WorkspaceSkipList, log_if_no_matches, should_visit_entry,
    unwrap_or_log_error,
};
use crate::stdlib::which::env::{self, EnvSnapshot};

/// Encapsulates the state and logic for collecting matching executables during
/// workspace traversal.
struct CollectionState {
    matches: Vec<Utf8PathBuf>,
    collect_all: bool,
}

impl CollectionState {
    fn new(collect_all: bool) -> Self {
        Self {
            matches: Vec::new(),
            collect_all,
        }
    }

    /// Process an entry and add it to matches if valid. Returns `true` if
    /// collection should stop (i.e., a match was found and `collect_all` is
    /// `false`).
    fn try_add(
        &mut self,
        entry: walkdir::DirEntry,
        command: &str,
        ctx: &WorkspaceMatchContext,
    ) -> Result<bool, Error> {
        if let Some(path) = match_workspace_entry(entry, command, ctx)? {
            self.matches.push(path);
            if !self.collect_all {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

pub(super) fn search_workspace(
    env: &EnvSnapshot,
    command: &str,
    collect_all: bool,
    skip_dirs: &WorkspaceSkipList,
) -> Result<Vec<Utf8PathBuf>, Error> {
    let match_ctx = WorkspaceMatchContext::new(command, env);
    let mut collector = CollectionState::new(collect_all);

    for walk_entry in WalkDir::new(&env.cwd)
        .follow_links(false)
        .max_depth(WORKSPACE_MAX_DEPTH)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| should_visit_entry(entry, skip_dirs))
    {
        let Some(entry) = unwrap_or_log_error(walk_entry, command) else {
            continue;
        };

        if collector.try_add(entry, command, &match_ctx)? {
            break;
        }
    }

    log_if_no_matches(&collector.matches, command, skip_dirs);

    Ok(collector.matches)
}

#[derive(Clone)]
struct WorkspaceMatchContext {
    command_lower: String,
    command_has_ext: bool,
    basenames: HashSet<String>,
}

fn match_workspace_entry(
    entry: walkdir::DirEntry,
    command: &str,
    ctx: &WorkspaceMatchContext,
) -> Result<Option<Utf8PathBuf>, Error> {
    if !entry.file_type().is_file() {
        return Ok(None);
    }

    let file_name = entry.file_name().to_string_lossy().to_ascii_lowercase();
    if !name_matches(&file_name, ctx) {
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

impl WorkspaceMatchContext {
    fn new(command: &str, env: &EnvSnapshot) -> Self {
        let command_lower = command.to_ascii_lowercase();
        let command_has_ext = command_lower.contains('.');
        let mut basenames = HashSet::new();

        if !command_has_ext {
            let candidates = env::candidate_paths(Utf8Path::new(""), &command_lower, env.pathext());
            for candidate in candidates {
                if let Some(name) = Utf8Path::new(candidate.as_str()).file_name() {
                    basenames.insert(name.to_ascii_lowercase());
                }
            }
        }

        Self {
            command_lower,
            command_has_ext,
            basenames,
        }
    }
}

fn name_matches(file_name: &str, ctx: &WorkspaceMatchContext) -> bool {
    if file_name == ctx.command_lower {
        return true;
    }
    if ctx.command_has_ext {
        return false;
    }
    ctx.basenames.contains(file_name)
}
