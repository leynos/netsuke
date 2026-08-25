//! Windows workspace traversal for the `which` fallback.

use std::collections::HashSet;

use camino::{Utf8Path, Utf8PathBuf};
use walkdir::WalkDir;

use super::super::is_executable;
use super::{
    WORKSPACE_MAX_DEPTH, WorkspaceSkipList, log_if_no_matches, should_visit_entry,
    unwrap_or_log_error,
};
use crate::stdlib::which::{
    env::{self, EnvSnapshot},
    resolve_error::ResolveError,
};

/// Encapsulates the state and logic for collecting matching executables during
/// workspace traversal.
struct CollectionState {
    /// Paths that matched the requested command during traversal.
    matches: Vec<Utf8PathBuf>,
    /// Whether traversal should retain every match instead of stopping early.
    collect_all: bool,
}

impl CollectionState {
    /// Create an empty collector with the requested collection mode.
    const fn new(collect_all: bool) -> Self {
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
    ) -> Result<bool, ResolveError> {
        if let Some(path) = match_workspace_entry(entry, command, ctx)? {
            self.matches.push(path);
            if !self.collect_all {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

/// Search the working directory tree for executable command matches.
///
/// # Errors
///
/// Returns a [`ResolveError`] when directory traversal fails, a workspace path
/// is not valid UTF-8, or an executable probe fails.
pub(super) fn search_workspace(
    env: &EnvSnapshot,
    command: &str,
    collect_all: bool,
    skip_dirs: &WorkspaceSkipList,
) -> Result<Vec<Utf8PathBuf>, ResolveError> {
    let match_ctx = WorkspaceMatchContext::new(command, env);
    let mut collector = CollectionState::new(collect_all);

    for walk_entry in WalkDir::new(&env.cwd)
        .follow_links(false)
        .max_depth(WORKSPACE_MAX_DEPTH)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| should_visit_entry(entry, skip_dirs))
    {
        let entry = unwrap_or_log_error(walk_entry)?;

        if collector.try_add(entry, command, &match_ctx)? {
            break;
        }
    }

    log_if_no_matches(&collector.matches, skip_dirs);

    Ok(collector.matches)
}

/// Store the Windows-specific command matching state for workspace traversal.
#[derive(Clone)]
struct WorkspaceMatchContext {
    /// Lowercase command name used for case-insensitive matching.
    command_lower: String,
    /// Whether the requested command already contains an extension.
    command_has_ext: bool,
    /// Lowercase executable basenames accepted through PATHEXT expansion.
    basenames: HashSet<String>,
}

/// Match one workspace entry and return it when it is an executable command.
///
/// # Errors
///
/// Returns a [`ResolveError`] when the entry path is not valid UTF-8 or the
/// executability probe fails.
fn match_workspace_entry(
    entry: walkdir::DirEntry,
    command: &str,
    ctx: &WorkspaceMatchContext,
) -> Result<Option<Utf8PathBuf>, ResolveError> {
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
        ResolveError::WorkspaceNonUtf8 {
            command: command.to_owned(),
            path: lossy_path.into_owned(),
        }
    })?;

    Ok(is_executable(&utf8)?.then_some(utf8))
}

impl WorkspaceMatchContext {
    /// Build matching state from a command and the captured environment.
    fn new(command: &str, env: &EnvSnapshot) -> Self {
        let command_lower = command.to_ascii_lowercase();
        let command_has_ext = command_lower.contains('.');
        let mut basenames = HashSet::new();

        if !command_has_ext {
            let candidates = env::candidate_paths(Utf8Path::new(""), &command_lower, env.pathext());
            basenames.extend(candidates.into_iter().filter_map(|candidate| {
                Utf8Path::new(candidate.as_str())
                    .file_name()
                    .map(str::to_ascii_lowercase)
            }));
        }

        Self {
            command_lower,
            command_has_ext,
            basenames,
        }
    }
}

/// Check whether a filename matches the command and PATHEXT candidates.
fn name_matches(file_name: &str, ctx: &WorkspaceMatchContext) -> bool {
    if file_name == ctx.command_lower {
        return true;
    }
    if ctx.command_has_ext {
        return false;
    }
    ctx.basenames.contains(file_name)
}
