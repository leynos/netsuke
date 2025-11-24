//! Tests for the which lookup helpers, covering PATH search, workspace
//! fallback, canonicalisation, and platform-specific PATHEXT behaviour.

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
fn search_workspace_returns_executable_and_skips_non_exec(workspace: TempWorkspace) -> Result<()> {
    let exec = write_exec(workspace.root(), "tool")?;
    let non_exec = workspace.root().join("tool2");
    fs::write(non_exec.as_std_path(), b"not exec").context("write non exec")?;

    #[cfg(windows)]
    let snapshot =
        EnvSnapshot::capture(Some(workspace.root())).expect("capture env for workspace search");
    #[cfg(windows)]
    let results = search_workspace(workspace.root(), "tool", false, &snapshot)?;
    #[cfg(not(windows))]
    let results = search_workspace(workspace.root(), "tool", false, ())?;
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

    #[cfg(windows)]
    let snapshot =
        EnvSnapshot::capture(Some(workspace.root())).expect("capture env for workspace search");
    #[cfg(windows)]
    let mut results = search_workspace(workspace.root(), "tool", true, &snapshot)?;
    #[cfg(not(windows))]
    let mut results = search_workspace(workspace.root(), "tool", true, ())?;
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

    #[cfg(windows)]
    let snapshot =
        EnvSnapshot::capture(Some(workspace.root())).expect("capture env for workspace search");
    #[cfg(windows)]
    let results = search_workspace(workspace.root(), "tool", false, &snapshot)?;
    #[cfg(not(windows))]
    let results = search_workspace(workspace.root(), "tool", false, ())?;
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
    #[cfg(windows)]
    let snapshot =
        EnvSnapshot::capture(Some(workspace.root())).expect("capture env for workspace search");
    #[cfg(windows)]
    let results = search_workspace(workspace.root(), "tool", false, &snapshot)?;
    #[cfg(not(windows))]
    let results = search_workspace(workspace.root(), "tool", false, ())?;
    ensure!(
        results == vec![exec],
        "expected readable executable despite blocked dir"
    );
    Ok(())
}

#[cfg(windows)]
#[rstest]
fn resolve_direct_appends_pathext(env: TempWorkspace) -> Result<()> {
    let base = env.root().join("tools").join("gradlew");
    fs::create_dir_all(base.parent().expect("tools dir").as_std_path()).context("mkdir tools")?;
    let exe = base.with_extension("bat");
    fs::write(exe.as_std_path(), b"@echo off\r\n").context("write stub")?;
    make_executable(&exe)?;

    let snapshot = EnvSnapshot {
        cwd: env.root.clone(),
        raw_path: None,
        raw_pathext: Some(".bat".into()),
        entries: vec![],
        pathext: vec![".bat".into()],
    };

    let matches = resolve_direct(".\\tools\\gradlew", &snapshot, &WhichOptions::default())?;

    ensure!(
        matches == vec![exe],
        "expected PATHEXT to expand direct path; got {matches:?}"
    );
    Ok(())
}
