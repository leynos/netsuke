//! Filesystem search utilities for resolving commands for the `which` feature.
//!
//! PATH lookup is deliberately ambient, so executable probes and
//! canonicalization cannot use the capability-based handles mandated elsewhere
//! in Netsuke.

use std::{fs, io};

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;

use super::options::CwdMode;

#[cfg(windows)]
use super::env;
use super::{
    env::EnvSnapshot,
    options::WhichOptions,
    resolve_error::{ResolveError, direct_not_found_error, not_found},
};
mod workspace;
use workspace::search_workspace;
pub(crate) use workspace::{WORKSPACE_SKIP_DIRS, WorkspaceSkipList};

/// Resolve `command` either as a direct path or by searching the environment's
/// PATH, optionally canonicalizing or collecting all matches.
///
/// When `options.all` is `true`, every executable candidate is returned;
/// otherwise resolution stops at the first match. The current working directory
/// is injected according to `options.cwd_mode`. Results are canonicalized when
/// requested, and cache-friendly options (such as `fresh`) are respected
/// upstream by the resolver.
///
/// # Errors
///
/// Propagates filesystem errors during lookup and canonicalization, and
/// returns `netsuke::jinja::which::not_found` when no candidate is discovered.
pub(super) fn lookup(
    command: &str,
    env: &EnvSnapshot,
    options: &WhichOptions,
    workspace_skips: &WorkspaceSkipList,
) -> Result<Vec<Utf8PathBuf>, ResolveError> {
    if is_direct_path(command) {
        return resolve_direct(command, env, options);
    }

    let dirs = env.resolved_dirs(options.cwd_mode);
    let mut matches = Vec::new();

    for dir in &dirs {
        let candidates = candidates_for_dir(env, dir, command);
        if push_matches(&mut matches, candidates, options.all)? {
            break;
        }
    }

    if matches.is_empty() {
        return handle_miss(HandleMissContext {
            env,
            command,
            options,
            dirs: &dirs,
            workspace_skips,
        });
    }

    if options.canonical {
        canonicalize(matches)
    } else {
        Ok(matches)
    }
}

/// Resolve a command that already looks like a path (absolute or relative),
/// returning all matching executable paths.
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
#[cfg(windows)]
pub(super) fn resolve_direct(
    command: &str,
    env: &EnvSnapshot,
    options: &WhichOptions,
) -> Result<Vec<Utf8PathBuf>, ResolveError> {
    let resolved = normalize_direct_path(command, env);
    let candidates = direct_candidates(&resolved, env);
    let mut matches = Vec::new();
    let _ = push_matches(&mut matches, candidates, options.all)?;
    if matches.is_empty() {
        return Err(direct_not_found_error(command, &resolved));
    }
    if options.canonical {
        canonicalize(matches)
    } else {
        Ok(matches)
    }
}

/// Resolve a path-like command to executable matches.
///
/// On POSIX the candidate must already be executable; canonicalization is
/// applied when requested in `options`. The result contains one direct-path
/// match on POSIX.
#[cfg(not(windows))]
pub(super) fn resolve_direct(
    command: &str,
    env: &EnvSnapshot,
    options: &WhichOptions,
) -> Result<Vec<Utf8PathBuf>, ResolveError> {
    let resolved = normalize_direct_path(command, env);
    if !is_executable(&resolved)? {
        return Err(direct_not_found_error(command, &resolved));
    }
    if options.canonical {
        canonicalize(vec![resolved])
    } else {
        Ok(vec![resolved])
    }
}

/// Resolve a path-like command against the working directory when relative.
fn normalize_direct_path(command: &str, env: &EnvSnapshot) -> Utf8PathBuf {
    let raw = Utf8Path::new(command);
    if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        env.cwd.join(raw)
    }
}

/// Build the candidate paths for a resolved direct path, on Windows.
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

/// Push executable candidates into `matches`, optionally short-circuiting when
/// only the first hit is required.
///
/// Returns `true` when at least one candidate was added and `collect_all` is
/// `false`, signalling to callers that the search can stop; returns `false`
/// otherwise.
///
/// # Errors
///
/// Returns a resolver error when checking whether a candidate is executable
/// fails for a reason other than the candidate being absent.
pub(super) fn push_matches(
    matches: &mut Vec<Utf8PathBuf>,
    candidates: Vec<Utf8PathBuf>,
    collect_all: bool,
) -> Result<bool, ResolveError> {
    for candidate in candidates {
        if !is_executable(&candidate)? {
            continue;
        }
        matches.push(candidate);
        if !collect_all {
            return Ok(true);
        }
    }
    Ok(false)
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

/// Build the candidate paths for a command within one directory.
fn candidates_for_dir(env: &EnvSnapshot, dir: &Utf8Path, command: &str) -> Vec<Utf8PathBuf> {
    #[cfg(windows)]
    {
        env::candidate_paths(dir, command, env.pathext())
    }
    #[cfg(not(windows))]
    {
        let _ = env;
        vec![dir.join(command)]
    }
}

/// Check whether `path` points to an executable file.
///
/// On Unix this requires at least one execute bit. On other platforms it only
/// verifies that the path exists and is a file.
///
/// # Errors
///
/// Returns a resolver error when metadata cannot be read for a reason other
/// than the path not existing.
pub(super) fn is_executable(path: &Utf8Path) -> Result<bool, ResolveError> {
    match fs::metadata(path.as_std_path()) {
        Ok(metadata) => Ok(is_executable_metadata(&metadata)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(ResolveError::IsExecutable {
            path: path.to_owned(),
            source,
        }),
    }
}

/// Whether metadata describes an executable file on Unix.
#[cfg(unix)]
fn is_executable_metadata(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
}

/// Whether metadata describes an executable file on non-Unix platforms.
#[cfg(not(unix))]
fn is_executable_metadata(metadata: &fs::Metadata) -> bool {
    metadata.is_file()
}

/// Context for handling a PATH-search miss.
#[derive(Clone, Copy)]
struct HandleMissContext<'a> {
    /// The environment snapshot the failed search ran against.
    env: &'a EnvSnapshot,
    /// The command that was not found.
    command: &'a str,
    /// The options the failed search ran with.
    options: &'a WhichOptions,
    /// The directories already searched, for the miss diagnostic.
    dirs: &'a [&'a Utf8Path],
    /// The skip list for the workspace fallback search.
    workspace_skips: &'a WorkspaceSkipList,
}

/// Handle a PATH-search miss, recursively searching the workspace only on request.
fn handle_miss(ctx: HandleMissContext<'_>) -> Result<Vec<Utf8PathBuf>, ResolveError> {
    if matches!(ctx.options.cwd_mode, CwdMode::WorkspaceRecursive) {
        let discovered =
            search_workspace(ctx.env, ctx.command, ctx.options.all, ctx.workspace_skips)?;
        if !discovered.is_empty() {
            return if ctx.options.canonical {
                canonicalize(discovered)
            } else {
                Ok(discovered)
            };
        }
    }

    Err(not_found(ctx.command, ctx.dirs, ctx.options.cwd_mode))
}

/// Canonicalize, de-duplicate, and UTF-8 validate discovered paths.
///
/// Returns an error when canonicalization fails or when any canonical path
/// cannot be represented as UTF-8.
pub(super) fn canonicalize(paths: Vec<Utf8PathBuf>) -> Result<Vec<Utf8PathBuf>, ResolveError> {
    let mut unique = IndexSet::new();
    let mut resolved = Vec::new();
    for path in paths {
        let canonical =
            fs::canonicalize(path.as_std_path()).map_err(|source| ResolveError::Canonicalize {
                path: path.clone(),
                source,
            })?;
        let utf8 =
            Utf8PathBuf::from_path_buf(canonical).map_err(|_| ResolveError::CanonicalizeNonUtf8)?;
        if unique.insert(utf8.clone()) {
            resolved.push(utf8);
        }
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests;
