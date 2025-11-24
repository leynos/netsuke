//! Workspace fallback search helpers for the `which` resolver.

#[cfg(windows)]
use std::collections::HashSet;

use camino::{Utf8Path, Utf8PathBuf};
use minijinja::{Error, ErrorKind};
use walkdir::WalkDir;

#[cfg(windows)]
use super::EnvSnapshot;
#[cfg(windows)]
use super::env;
use super::is_executable;

pub(super) fn search_workspace(
    cwd: &Utf8Path,
    command: &str,
    collect_all: bool,
    #[cfg(windows)] env: &EnvSnapshot,
    #[cfg(not(windows))] _env: (),
) -> Result<Vec<Utf8PathBuf>, Error> {
    #[cfg(windows)]
    let match_ctx = prepare_workspace_match(command, env);
    #[cfg(not(windows))]
    let match_ctx = ();

    let entries = WalkDir::new(cwd)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(should_visit_entry)
        .filter_map(|walk_entry| {
            walk_entry
                .map_err(|err| {
                    tracing::debug!(
                        %command,
                        error = %err,
                        "skipping unreadable workspace entry during which fallback"
                    );
                    err
                })
                .ok()
        });

    collect_workspace_matches(entries, command, collect_all, match_ctx)
}

/// Accumulates executable matches from workspace traversal, stopping early
/// when `collect_all` is `false`. The iterator supplies already-filtered
/// directory entries; platform-specific match contexts ensure consistent
/// filename matching semantics.
fn collect_workspace_matches(
    entries: impl Iterator<Item = walkdir::DirEntry>,
    command: &str,
    collect_all: bool,
    #[cfg(windows)] match_ctx: WorkspaceMatchContext,
    #[cfg(not(windows))] match_ctx: (),
) -> Result<Vec<Utf8PathBuf>, Error> {
    let mut matches = Vec::new();

    for entry in entries {
        #[cfg(windows)]
        let maybe_match = process_workspace_entry(entry, command, &match_ctx)?;
        #[cfg(not(windows))]
        let maybe_match = process_workspace_entry(entry, command, match_ctx)?;

        if let Some(path) = maybe_match {
            matches.push(path);
            if !collect_all {
                break;
            }
        }
    }

    Ok(matches)
}

const WORKSPACE_SKIP_DIRS: &[&str] = &[".git", "target"];

/// Allow traversal for all files and directories except heavy/VCS roots to
/// keep workspace scans fast.
fn should_visit_entry(entry: &walkdir::DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    !WORKSPACE_SKIP_DIRS.iter().any(|skip| name == *skip)
}

/// Process a single `walkdir::DirEntry`: ensure it is a file, apply the
/// platform-specific filename match, convert the path to UTF-8 (erroring on
/// non-UTF-8 components), and return `Some(path)` only when the entry is
/// executable; otherwise return `None`.
fn process_workspace_entry(
    entry: walkdir::DirEntry,
    command: &str,
    #[cfg(windows)] ctx: &WorkspaceMatchContext,
    #[cfg(not(windows))] ctx: (),
) -> Result<Option<Utf8PathBuf>, Error> {
    if !entry.file_type().is_file() {
        return Ok(None);
    }
    #[cfg(windows)]
    let matches_entry = workspace_entry_matches(&entry, ctx);
    #[cfg(not(windows))]
    let matches_entry = workspace_entry_matches(&entry, command, ctx);
    if !matches_entry {
        return Ok(None);
    }
    let path = entry.into_path();
    let utf8 = Utf8PathBuf::from_path_buf(path).map_err(|path_buf| {
        let lossy_path = path_buf.to_string_lossy();
        Error::new(
            ErrorKind::InvalidOperation,
            format!(
                "workspace path contains non-UTF-8 components while resolving command '{command}': {lossy_path}"
            ),
        )
    })?;
    Ok(is_executable(&utf8).then_some(utf8))
}

#[cfg(windows)]
/// Windows-specific match context for case-insensitive filename matching.
///
/// Encapsulates normalised command state for workspace traversal:
/// - `command_lower`: lowercased command name.
/// - `command_has_ext`: whether the supplied command already includes a file
///   extension.
/// - `basenames`: PATHEXT-expanded candidate filenames for extension-less
///   commands, stored in lowercase for case-insensitive comparisons.
#[derive(Clone)]
struct WorkspaceMatchContext {
    command_lower: String,
    command_has_ext: bool,
    basenames: HashSet<String>,
}

#[cfg(windows)]
/// Perform case-insensitive filename matching with PATHEXT expansion.
///
/// Returns `true` when the entry's lowercased filename matches the command
/// directly or—when the command lacks an extension—any PATHEXT-expanded
/// basename candidate.
fn workspace_entry_matches(entry: &walkdir::DirEntry, ctx: &WorkspaceMatchContext) -> bool {
    let file_name = entry.file_name().to_string_lossy().to_ascii_lowercase();
    if file_name == ctx.command_lower {
        return true;
    }
    if ctx.command_has_ext {
        return false;
    }
    ctx.basenames.contains(&file_name)
}

#[cfg(not(windows))]
/// Perform exact case-sensitive filename matching.
///
/// Returns `true` when the entry's filename matches the command string.
fn workspace_entry_matches(entry: &walkdir::DirEntry, command: &str, _ctx: ()) -> bool {
    let file_name = entry.file_name().to_string_lossy();
    file_name == command
}

#[cfg(windows)]
/// Initialise Windows match context by normalising the command and expanding
/// PATHEXT.
///
/// Lowercases the command, records whether it already contains an extension,
/// and—when extension-less—derives candidate basenames by applying PATHEXT
/// suffixes via `env::candidate_paths`. All basenames are stored in lowercase
/// to enable case-insensitive comparisons during workspace traversal.
fn prepare_workspace_match(command: &str, env: &EnvSnapshot) -> WorkspaceMatchContext {
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

    WorkspaceMatchContext {
        command_lower,
        command_has_ext,
        basenames,
    }
}
