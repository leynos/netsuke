//! Filesystem search utilities for resolving commands for the `which` feature.

use std::fs;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;
use minijinja::{Error, ErrorKind};
use walkdir::WalkDir;

use super::options::CwdMode;

#[cfg(windows)]
use super::env;
use super::{
    env::EnvSnapshot,
    error::{direct_not_found, not_found_error},
    options::WhichOptions,
};

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

fn search_workspace(
    cwd: &Utf8Path,
    command: &str,
    collect_all: bool,
    env: &EnvSnapshot,
) -> Result<Vec<Utf8PathBuf>, Error> {
    const SKIP_DIRS: &[&str] = &[".git", "target"];
    let mut matches = Vec::new();
    let walker = WalkDir::new(cwd)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| {
            let ft = entry.file_type();
            if ft.is_dir() {
                let name = entry.file_name().to_string_lossy();
                !SKIP_DIRS.iter().any(|skip| name == *skip)
            } else {
                true
            }
        });

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
        if !entry.file_type().is_file() {
            continue;
        }
        if !workspace_entry_matches(&entry, command, env) {
            continue;
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
        if !is_executable(&utf8) {
            continue;
        }
        matches.push(utf8);
        if !collect_all {
            break;
        }
    }
    Ok(matches)
}

fn workspace_entry_matches(entry: &walkdir::DirEntry, command: &str, env: &EnvSnapshot) -> bool {
    #[cfg(not(windows))]
    let _ = env;
    #[cfg(windows)]
    {
        let file_name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        let command_lower = command.to_ascii_lowercase();

        if file_name == command_lower {
            return true;
        }

        if command_lower.contains('.') {
            return false;
        }

        let candidates = env::candidate_paths(Utf8Path::new(""), &command_lower, env.pathext());
        candidates
            .iter()
            .filter_map(|candidate| Utf8Path::new(candidate.as_str()).file_name())
            .any(|name| name.to_ascii_lowercase() == file_name)
    }
    #[cfg(not(windows))]
    {
        let file_name = entry.file_name().to_string_lossy();
        file_name == command
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
