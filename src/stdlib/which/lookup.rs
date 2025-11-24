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

fn handle_miss(
    env: &EnvSnapshot,
    command: &str,
    options: &WhichOptions,
    dirs: &[Utf8PathBuf],
) -> Result<Vec<Utf8PathBuf>, Error> {
    let path_empty = env.raw_path.as_ref().is_none_or(|path| path.is_empty());

    if path_empty && !matches!(options.cwd_mode, CwdMode::Never) {
        let discovered = search_workspace(&env.cwd, command, options.all)?;
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
        if entry.file_name() != command {
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
mod tests {
    use super::*;
    use anyhow::{Context, Result, anyhow, ensure};
    use rstest::{fixture, rstest};
    use std::fs;
    use tempfile::TempDir;

    struct TempWorkspace {
        root: Utf8PathBuf,
        _tempdir: TempDir,
    }

    impl TempWorkspace {
        fn new() -> Result<Self> {
            let tempdir = TempDir::new().context("create tempdir")?;
            let root = Utf8PathBuf::from_path_buf(tempdir.path().to_path_buf())
                .map_err(|path| anyhow!("utf8 path required, got {:?}", path))?;
            Ok(Self {
                root,
                _tempdir: tempdir,
            })
        }

        fn root(&self) -> &Utf8Path {
            self.root.as_path()
        }
    }

    #[fixture]
    fn workspace() -> TempWorkspace {
        TempWorkspace::new().expect("create utf8 temp workspace")
    }

    #[cfg(unix)]
    fn make_executable(path: &Utf8Path) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path.as_std_path())
            .context("stat exec")?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path.as_std_path(), perms).context("chmod exec")
    }

    #[cfg(not(unix))]
    fn make_executable(_path: &Utf8Path) -> Result<()> {
        Ok(())
    }

    fn write_exec(root: &Utf8Path, name: &str) -> Result<Utf8PathBuf> {
        let path = root.join(name);
        fs::write(path.as_std_path(), b"#!/bin/sh\n").context("write exec stub")?;
        make_executable(&path)?;
        Ok(path)
    }

    #[rstest]
    fn search_workspace_returns_executable_and_skips_non_exec(
        workspace: TempWorkspace,
    ) -> Result<()> {
        let exec = write_exec(workspace.root(), "tool")?;
        let non_exec = workspace.root().join("tool2");
        fs::write(non_exec.as_std_path(), b"not exec").context("write non exec")?;

        let results = search_workspace(workspace.root(), "tool", false)?;
        ensure!(
            results == vec![exec],
            "expected executable to be discovered"
        );
        Ok(())
    }

    #[rstest]
    fn search_workspace_collects_all_matches(workspace: TempWorkspace) -> Result<()> {
        let first = write_exec(workspace.root(), "tool")?;
        let subdir = workspace.root().join("bin");
        fs::create_dir_all(subdir.as_std_path()).context("mkdir bin")?;
        let second = write_exec(subdir.as_path(), "tool")?;

        let mut results = search_workspace(workspace.root(), "tool", true)?;
        results.sort();
        let mut expected = vec![first, second];
        expected.sort();
        ensure!(
            results == expected,
            "expected both executables to be returned"
        );
        Ok(())
    }

    #[rstest]
    fn search_workspace_skips_heavy_directories(workspace: TempWorkspace) -> Result<()> {
        let heavy = workspace.root().join("target");
        fs::create_dir_all(heavy.as_std_path()).context("mkdir target")?;
        write_exec(heavy.as_path(), "tool")?;

        let results = search_workspace(workspace.root(), "tool", false)?;
        ensure!(results.is_empty(), "expected target/ to be skipped");
        Ok(())
    }

    #[cfg(unix)]
    #[rstest]
    fn search_workspace_ignores_unreadable_entries(workspace: TempWorkspace) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let blocked = workspace.root().join("blocked");
        fs::create_dir_all(blocked.as_std_path()).context("mkdir blocked")?;
        let mut perms = fs::metadata(blocked.as_std_path())
            .context("stat blocked")?
            .permissions();
        perms.set_mode(0o000);
        fs::set_permissions(blocked.as_std_path(), perms).context("chmod blocked")?;

        let exec = write_exec(workspace.root(), "tool")?;
        let results = search_workspace(workspace.root(), "tool", false)?;
        ensure!(
            results == vec![exec],
            "expected readable executable despite blocked dir"
        );
        Ok(())
    }
}
