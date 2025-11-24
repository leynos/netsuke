//! Workspace fallback search helpers for the `which` resolver.

#[cfg(windows)]
use std::collections::HashSet;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;
use minijinja::{Error, ErrorKind};
use walkdir::WalkDir;

#[cfg(windows)]
use super::EnvSnapshot;
#[cfg(windows)]
use super::env;
use super::is_executable;

/// Default set of workspace directories to skip during fallback scans.
pub(crate) const DEFAULT_WORKSPACE_SKIP_DIRS: &[&str] =
    &[".git", "target", "node_modules", ".idea", ".vscode"];

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct WorkspaceSkipList {
    dirs: Arc<Vec<String>>,
}

/// Inputs for scanning the workspace during `which` fallback resolution.
#[derive(Clone, Copy)]
pub(super) struct WorkspaceSearch<'a> {
    pub(super) cwd: &'a Utf8Path,
    pub(super) command: &'a str,
    pub(super) collect_all: bool,
    pub(super) skip_dirs: &'a WorkspaceSkipList,
}

impl WorkspaceSkipList {
    /// Build a skip list from the provided directory names, normalising for the
    /// host platform and removing duplicates.
    pub(crate) fn from_names(names: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        let mut dedup = IndexSet::new();
        for name in names {
            let trimmed = name.as_ref().trim();
            if trimmed.is_empty() {
                continue;
            }
            dedup.insert(normalise_dir_name(trimmed));
        }
        let mut dirs: Vec<_> = dedup.into_iter().collect();
        dirs.sort();
        Self {
            dirs: Arc::new(dirs),
        }
    }

    /// Return `true` when a directory should be skipped during traversal.
    pub(crate) fn should_skip(&self, name: &str) -> bool {
        let normalised = normalise_dir_name(name);
        self.dirs.contains(&normalised)
    }
}

impl Default for WorkspaceSkipList {
    fn default() -> Self {
        Self::from_names(DEFAULT_WORKSPACE_SKIP_DIRS.iter().copied())
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

/// Recursively search the workspace rooted at `cwd` for executables matching
/// `command`.
///
/// - `cwd`: workspace root to traverse (symlinks are not followed).
/// - `command`: name to match (Windows: case-insensitive with `PATHEXT`
///   expansion; other platforms: exact case-sensitive filename match).
/// - `collect_all`: when `true`, return every match; otherwise stop after the
///   first executable.
/// - `skip_dirs`: directory basenames to omit during traversal.
/// - `env`: provided only on Windows to supply `PATHEXT` for matching.
///
/// Skips unreadable entries, ignores heavy/VCS directories via
/// `should_visit_entry`, and returns `Ok(Vec<Utf8PathBuf>)` containing the
/// discovered executables or an `Error` if UTF-8 conversion fails.
pub(super) fn search_workspace(
    search: WorkspaceSearch<'_>,
    #[cfg(windows)] env: &EnvSnapshot,
    #[cfg(not(windows))] _env: (),
) -> Result<Vec<Utf8PathBuf>, Error> {
    #[cfg(windows)]
    let match_ctx = prepare_workspace_match(search.command, env);
    #[cfg(not(windows))]
    let match_ctx = ();

    let entries = WalkDir::new(search.cwd)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| should_visit_entry(entry, search.skip_dirs))
        .filter_map(|walk_entry| {
            walk_entry
                .map_err(|err| {
                    tracing::debug!(
                        command = %search.command,
                        error = %err,
                        "skipping unreadable workspace entry during which fallback"
                    );
                    err
                })
                .ok()
        });

    collect_workspace_matches(entries, search.command, search.collect_all, match_ctx)
}

/// Collect executable matches from workspace traversal.
///
/// Parameters:
/// - `entries`: iterator of `walkdir::DirEntry` values to inspect.
/// - `command`: command name used for platform-specific filename matching.
/// - `collect_all`: when `false`, stops after the first executable match.
/// - `match_ctx`: on Windows, a `WorkspaceMatchContext`; on other platforms,
///   the unit type to align signatures.
///
/// Returns a `Result<Vec<Utf8PathBuf>, Error>` containing matched executable
/// paths or an error when UTF-8 conversion fails.
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

/// Allow traversal for all files and directories except heavy/VCS roots to
/// keep workspace scans fast.
fn should_visit_entry(entry: &walkdir::DirEntry, skip_dirs: &WorkspaceSkipList) -> bool {
    if !entry.file_type().is_dir() {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    !skip_dirs.should_skip(&name)
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
