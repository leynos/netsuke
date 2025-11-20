use std::fs;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;
use minijinja::{Error, ErrorKind};

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
        return Err(not_found_error(command, &dirs, options.cwd_mode));
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
    if !is_executable(&resolved) {
        return Err(direct_not_found(command, &resolved));
    }
    if options.canonical {
        canonicalise(vec![resolved])
    } else {
        Ok(vec![resolved])
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
