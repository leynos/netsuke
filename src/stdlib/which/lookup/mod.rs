//! Filesystem search utilities for resolving commands for the `which` feature.

use std::fs;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;
use minijinja::{Error, ErrorKind};
#[cfg(windows)]
use std::collections::HashSet;
use walkdir::WalkDir;

use super::options::CwdMode;

#[cfg(windows)]
use super::env;
use super::{
    env::EnvSnapshot,
    error::{direct_not_found, not_found_error},
    options::WhichOptions,
};

/// Resolve `command` either as a direct path or by searching the environment's
/// PATH, optionally canonicalising or collecting all matches.
///
/// When `options.all` is `true`, every executable candidate is returned;
/// otherwise resolution stops at the first match. The current working directory
/// is injected according to `options.cwd_mode`. Results are canonicalised when
/// requested, and cache-friendly options (such as `fresh`) are respected
/// upstream by the resolver.
///
/// # Errors
///
/// Propagates filesystem errors during lookup and canonicalisation, and
/// returns `netsuke::jinja::which::not_found` when no candidate is discovered.
pub(super) fn lookup(
    command: &str,
    env: &EnvSnapshot,
    options: &WhichOptions,
) -> Result<Vec<Utf8PathBuf>, Error> {
    if is_direct_path(command) {
        return resolve_direct(command, env, options);
    }

    let dirs = env.resolved_dirs(options.cwd_mode);
    let mut matches = Vec::new();

    #[cfg(windows)]
    let suffixes = env.pathext();

    for dir in &dirs {
        #[cfg(windows)]
        let candidates = env::candidate_paths(dir, command, suffixes);
        #[cfg(not(windows))]
        let candidates = vec![dir.join(command)];

        if push_matches(&mut matches, candidates, options.all) {
            break;
        }
    }

    if matches.is_empty() {
        return handle_miss(env, command, options, &dirs);
    }

    if options.canonical {
        canonicalise(matches)
    } else {
        Ok(matches)
    }
}

/// Resolve a command that already looks like a path (absolute or relative).
///
/// On Windows this honours `PATHEXT` when the path is missing an extension so
/// callers can pass `.\gradlew` and still resolve `gradlew.bat`. On POSIX the
/// candidate must already be executable. Canonicalisation is applied when
/// requested in `options`.
///
/// # Errors
///
/// Returns `netsuke::jinja::which::not_found` when no matching executable is
/// discovered.
pub(super) fn resolve_direct(
    command: &str,
    env: &EnvSnapshot,
    options: &WhichOptions,
) -> Result<Vec<Utf8PathBuf>, Error> {
    let raw = Utf8Path::new(command);
    let resolved = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        env.cwd.join(raw)
    };
    #[cfg(windows)]
    {
        resolve_direct_windows(command, &resolved, env, options)
    }
    #[cfg(not(windows))]
    {
        resolve_direct_posix(command, &resolved, options)
    }
}

#[cfg(windows)]
fn resolve_direct_windows(
    command: &str,
    resolved: &Utf8PathBuf,
    env: &EnvSnapshot,
    options: &WhichOptions,
) -> Result<Vec<Utf8PathBuf>, Error> {
    let candidates = direct_candidates(resolved, env);
    let mut matches = Vec::new();
    let _ = push_matches(&mut matches, candidates, options.all);
    if matches.is_empty() {
        return Err(direct_not_found(command, resolved));
    }
    if options.canonical {
        canonicalise(matches)
    } else {
        Ok(matches)
    }
}

#[cfg(windows)]
fn direct_candidates(resolved: &Utf8PathBuf, env: &EnvSnapshot) -> Vec<Utf8PathBuf> {
    if resolved.extension().is_some() {
        vec![resolved.clone()]
    } else {
        env.pathext()
            .iter()
            .map(|ext| {
                let mut candidate = resolved.as_str().to_owned();
                candidate.push_str(ext);
                Utf8PathBuf::from(candidate)
            })
            .collect()
    }
}

#[cfg(not(windows))]
fn resolve_direct_posix(
    command: &str,
    resolved: &Utf8PathBuf,
    options: &WhichOptions,
) -> Result<Vec<Utf8PathBuf>, Error> {
    if !is_executable(resolved) {
        return Err(direct_not_found(command, resolved));
    }
    if options.canonical {
        canonicalise(vec![resolved.clone()])
    } else {
        Ok(vec![resolved.clone()])
    }
}

/// Push executable candidates into `matches`, optionally short-circuiting when
/// only the first hit is required.
///
/// Returns `true` when at least one candidate was added and `collect_all` is
/// `false`, signalling to callers that the search can stop; returns `false`
/// otherwise.
pub(super) fn push_matches(
    matches: &mut Vec<Utf8PathBuf>,
    candidates: Vec<Utf8PathBuf>,
    collect_all: bool,
) -> bool {
    for candidate in candidates {
        if !is_executable(&candidate) {
            continue;
        }
        matches.push(candidate);
        if !collect_all {
            return true;
        }
    }
    false
}

/// Return `true` when the command string already includes path separators or,
/// on Windows, a drive letter, meaning PATH traversal should be skipped.
pub(super) fn is_direct_path(command: &str) -> bool {
    #[cfg(windows)]
    {
        command.contains(['\\', '/', ':'])
    }
    #[cfg(not(windows))]
    {
        command.contains('/')
    }
}

/// Check whether `path` points to an executable file.
///
/// On Unix this requires at least one execute bit. On other platforms it only
/// verifies that the path exists and is a file.
pub(super) fn is_executable(path: &Utf8Path) -> bool {
    fs::metadata(path.as_std_path())
        .is_ok_and(|metadata| metadata.is_file() && has_execute_permission(&metadata))
}

fn handle_miss(
    env: &EnvSnapshot,
    command: &str,
    options: &WhichOptions,
    dirs: &[Utf8PathBuf],
) -> Result<Vec<Utf8PathBuf>, Error> {
    let path_empty = env.raw_path.as_ref().is_none_or(|path| path.is_empty());

    if path_empty && !matches!(options.cwd_mode, CwdMode::Never) {
        let discovered = search_workspace(&env.cwd, command, options.all, env)?;
        if !discovered.is_empty() {
            return if options.canonical {
                canonicalise(discovered)
            } else {
                Ok(discovered)
            };
        }
    }

    Err(not_found_error(command, dirs, options.cwd_mode))
}

/// Walk the workspace looking for executables when PATH is empty and the
/// resolver is allowed to consult the current directory.
fn search_workspace(
    cwd: &Utf8Path,
    command: &str,
    collect_all: bool,
    env: &EnvSnapshot,
) -> Result<Vec<Utf8PathBuf>, Error> {
    let mut matches = Vec::new();
    #[cfg(not(windows))]
    let _ = env;
    #[cfg(windows)]
    let match_ctx = prepare_workspace_match(command, env);
    #[cfg(not(windows))]
    let match_ctx = ();
    let walker = WalkDir::new(cwd)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(should_visit_entry);

    for walk_entry in walker {
        let entry = match walk_entry {
            Ok(value) => value,
            Err(err) => {
                tracing::debug!(
                    %command,
                    error = %err,
                    "skipping unreadable workspace entry during which fallback"
                );
                continue;
            }
        };
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

fn should_visit_entry(entry: &walkdir::DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    !WORKSPACE_SKIP_DIRS.iter().any(|skip| name == *skip)
}

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
    let matches_entry = workspace_entry_matches(&entry, command, ctx);
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

fn workspace_entry_matches(
    entry: &walkdir::DirEntry,
    command: &str,
    #[cfg(windows)] ctx: &WorkspaceMatchContext,
    #[cfg(not(windows))] _ctx: (),
) -> bool {
    #[cfg(windows)]
    {
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
    {
        let file_name = entry.file_name().to_string_lossy();
        file_name == command
    }
}

#[cfg(windows)]
struct WorkspaceMatchContext {
    command_lower: String,
    command_has_ext: bool,
    basenames: HashSet<String>,
}

#[cfg(windows)]
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

#[cfg(unix)]
fn has_execute_permission(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn has_execute_permission(metadata: &fs::Metadata) -> bool {
    metadata.is_file()
}

/// Canonicalise, de-duplicate, and UTF-8 validate discovered paths.
///
/// Returns an error when canonicalisation fails or when any canonical path
/// cannot be represented as UTF-8.
pub(super) fn canonicalise(paths: Vec<Utf8PathBuf>) -> Result<Vec<Utf8PathBuf>, Error> {
    let mut unique = IndexSet::new();
    let mut resolved = Vec::new();
    for path in paths {
        let canonical = fs::canonicalize(path.as_std_path()).map_err(|err| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("failed to canonicalise '{path}': {err}"),
            )
        })?;
        let utf8 = Utf8PathBuf::from_path_buf(canonical).map_err(|_| {
            Error::new(
                ErrorKind::InvalidOperation,
                "canonical path contains non-UTF-8 characters",
            )
        })?;
        if unique.insert(utf8.clone()) {
            resolved.push(utf8);
        }
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests;
