//! Tests for the which lookup helpers, covering PATH search, workspace
//! fallback, canonicalization, and platform-specific PATHEXT behaviour.

use super::*;
use anyhow::{Context, Result, anyhow, ensure};
use rstest::{fixture, rstest};
use tempfile::TempDir;
use test_support::exec::write_exec;
use test_support::fs as test_fs;

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
fn workspace() -> Result<TempWorkspace> {
    TempWorkspace::new().context("create utf8 temp workspace")
}

#[rstest]
fn search_workspace_returns_executable_and_skips_non_exec(
    #[from(workspace)] workspace_res: Result<TempWorkspace>,
) -> Result<()> {
    let workspace = workspace_res?;
    let exec = write_exec(workspace.root().as_std_path(), "tool")?;
    let non_exec = workspace.root().join("tool2");
    test_fs::write(non_exec.as_std_path(), b"not exec").context("write non exec")?;

    let path_value = std::ffi::OsString::from(workspace.root().as_str());
    let snapshot = EnvSnapshot::capture(Some(workspace.root()), Some(path_value.as_os_str()))
        .expect("capture env for workspace search");
    let results = search_workspace(&snapshot, "tool", false, &WorkspaceSkipList::default())?;
    ensure!(
        results == vec![exec],
        "expected executable to be discovered"
    );
    Ok(())
}

#[rstest]
fn search_workspace_collects_all_matches(
    #[from(workspace)] workspace_res: Result<TempWorkspace>,
) -> Result<()> {
    let workspace = workspace_res?;
    let first = write_exec(workspace.root().as_std_path(), "tool")?;
    let subdir = workspace.root().join("bin");
    test_fs::create_dir_all(subdir.as_std_path()).context("mkdir bin")?;
    let second = write_exec(subdir.as_std_path(), "tool")?;

    let path_value = std::ffi::OsString::from(workspace.root().as_str());
    let snapshot = EnvSnapshot::capture(Some(workspace.root()), Some(path_value.as_os_str()))
        .expect("capture env for workspace search");
    let mut results = search_workspace(&snapshot, "tool", true, &WorkspaceSkipList::default())?;
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
fn search_workspace_skips_heavy_directories(
    #[from(workspace)] workspace_res: Result<TempWorkspace>,
) -> Result<()> {
    let workspace = workspace_res?;
    let heavy = workspace.root().join("target");
    test_fs::create_dir_all(heavy.as_std_path()).context("mkdir target")?;
    write_exec(heavy.as_std_path(), "tool")?;

    let path_value = std::ffi::OsString::from(workspace.root().as_str());
    let snapshot = EnvSnapshot::capture(Some(workspace.root()), Some(path_value.as_os_str()))
        .expect("capture env for workspace search");
    let results = search_workspace(&snapshot, "tool", false, &WorkspaceSkipList::default())?;
    ensure!(results.is_empty(), "expected target/ to be skipped");
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn search_workspace_surfaces_unreadable_entries(
    #[from(workspace)] workspace_res: Result<TempWorkspace>,
) -> Result<()> {
    let workspace = workspace_res?;
    let blocked = workspace.root().join("blocked");
    test_fs::create_dir_all(blocked.as_std_path()).context("mkdir blocked")?;
    test_fs::set_mode(blocked.as_std_path(), 0o000).context("chmod blocked")?;

    let path_value = std::ffi::OsString::from(workspace.root().as_str());
    let snapshot = EnvSnapshot::capture(Some(workspace.root()), Some(path_value.as_os_str()))
        .expect("capture env for workspace search");
    let result = search_workspace(&snapshot, "tool", false, &WorkspaceSkipList::default());

    // Restore permissions before asserting so the tempdir can always be cleaned
    // up, even if `search_workspace` unexpectedly succeeds and the assertion
    // below panics.
    test_fs::set_mode(blocked.as_std_path(), 0o700).context("restore blocked")?;

    let err = result.expect_err("unreadable workspace entries should fail");
    ensure!(
        matches!(err, ResolveError::WalkDir { .. }),
        "expected walkdir error, got {err:?}"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn path_with_invalid_utf8_triggers_args_error(
    #[from(workspace)] workspace_res: Result<TempWorkspace>,
) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;

    use crate::localization::{self, keys};

    let workspace = workspace_res?;
    let invalid_path = std::ffi::OsStr::from_bytes(b"/bin:\xFF");
    let err = EnvSnapshot::capture(Some(workspace.root()), Some(invalid_path))
        .expect_err("invalid PATH should fail EnvSnapshot::capture");

    let details = localization::message(keys::STDLIB_WHICH_PATH_ENTRY_NON_UTF8)
        .with_arg("index", 1)
        .to_string();

    match err {
        ResolveError::Args { detail } => ensure!(
            detail == details,
            "expected PATH parsing detail, got: {detail}"
        ),
        other => return Err(anyhow!("expected argument error, got: {other:?}")),
    }

    Ok(())
}

#[rstest]
fn relative_path_entries_resolve_against_cwd(
    #[from(workspace)] workspace_res: Result<TempWorkspace>,
) -> Result<()> {
    let workspace = workspace_res?;
    let bin = workspace.root().join("bin");
    let tools = workspace.root().join("tools");
    test_fs::create_dir_all(bin.as_std_path()).context("mkdir bin")?;
    test_fs::create_dir_all(tools.as_std_path()).context("mkdir tools")?;

    let path_value = std::env::join_paths([
        workspace.root().as_std_path(),
        std::path::Path::new("bin"),
        std::path::Path::new("tools"),
    ])
    .context("join PATH entries")?;

    let snapshot = EnvSnapshot::capture(Some(workspace.root()), Some(path_value.as_os_str()))
        .context("capture env with relative PATH entries")?;
    let resolved_dirs = snapshot.resolved_dirs(CwdMode::Never);

    ensure!(
        resolved_dirs.contains(&bin),
        "resolved_dirs missing bin: {resolved_dirs:?}"
    );
    ensure!(
        resolved_dirs.contains(&tools),
        "resolved_dirs missing tools: {resolved_dirs:?}"
    );

    Ok(())
}

#[cfg(windows)]
#[rstest]
fn pathext_empty_uses_default_fallback(
    #[from(workspace)] workspace_res: Result<TempWorkspace>,
) -> Result<()> {
    use test_support::env::VarGuard;

    let workspace = workspace_res?;
    let _pathext_guard = VarGuard::set("PATHEXT", std::ffi::OsStr::new(""));
    let path_value = std::ffi::OsString::from(workspace.root().as_str());

    let snapshot = EnvSnapshot::capture(Some(workspace.root()), Some(path_value.as_os_str()))
        .context("capture env for empty PATHEXT")?;
    let pathexts = snapshot.pathext();

    ensure!(
        pathexts.iter().any(|ext| ext.eq_ignore_ascii_case(".com")),
        "default PATHEXT should include .COM",
    );
    ensure!(
        pathexts.iter().any(|ext| ext.eq_ignore_ascii_case(".exe")),
        "default PATHEXT should include .EXE",
    );

    Ok(())
}

#[cfg(windows)]
#[rstest]
fn pathext_without_leading_dots_is_normalised_and_deduplicated(
    #[from(workspace)] workspace_res: Result<TempWorkspace>,
) -> Result<()> {
    use test_support::env::VarGuard;

    let workspace = workspace_res?;
    let _pathext_guard = VarGuard::set("PATHEXT", std::ffi::OsStr::new("COM;EXE;EXE; .BAT ;bat"));
    let path_value = std::ffi::OsString::from(workspace.root().as_str());

    let snapshot = EnvSnapshot::capture(Some(workspace.root()), Some(path_value.as_os_str()))?;
    let mut pathexts = snapshot.pathext().to_vec();
    pathexts.sort_unstable_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

    let contains_ci = |needle: &str| pathexts.iter().any(|ext| ext.eq_ignore_ascii_case(needle));

    ensure!(contains_ci(".COM"), "PATHEXT should include .COM");
    ensure!(contains_ci(".EXE"), "PATHEXT should include .EXE");
    ensure!(contains_ci(".BAT"), "PATHEXT should include .BAT");

    let mut lower: Vec<String> = pathexts
        .iter()
        .map(|ext| ext.to_ascii_lowercase())
        .collect();
    lower.sort_unstable();
    lower.dedup();
    ensure!(
        lower.len() == pathexts.len(),
        "PATHEXT entries should be deduplicated: {pathexts:?}"
    );

    Ok(())
}

#[cfg(unix)]
#[rstest]
fn direct_path_not_executable_raises_direct_not_found(
    #[from(workspace)] workspace_res: Result<TempWorkspace>,
) -> Result<()> {
    let workspace = workspace_res?;
    let script = workspace.root().join("script.sh");
    test_fs::write(script.as_std_path(), "#!/bin/sh\necho test\n").context("write script")?;
    test_fs::set_mode(script.as_std_path(), 0o644).context("chmod script")?;

    let path_value = std::ffi::OsString::from(workspace.root().as_str());
    let snapshot = EnvSnapshot::capture(Some(workspace.root()), Some(path_value.as_os_str()))
        .context("capture env for direct path")?;

    let err = resolve_direct(script.as_str(), &snapshot, &WhichOptions::default())
        .expect_err("non-executable direct path should fail");

    match err {
        ResolveError::DirectNotFound { command, path } => {
            ensure!(
                command == script.as_str(),
                "expected direct command payload, got: {command}"
            );
            ensure!(path == script, "expected direct path payload, got: {path}");
        }
        other => return Err(anyhow!("expected direct not found error, got: {other:?}")),
    }

    Ok(())
}

#[cfg(windows)]
#[rstest]
fn resolve_direct_appends_pathext(workspace: Result<TempWorkspace>) -> Result<()> {
    use test_support::exec::make_executable;

    let env = workspace?;
    let base = env.root().join("tools").join("gradlew");
    let tools_dir = base
        .parent()
        .ok_or_else(|| anyhow!("tools dir missing from {base}"))?;
    test_fs::create_dir_all(tools_dir.as_std_path()).context("mkdir tools")?;
    let exe = base.with_extension("bat");
    test_fs::write(exe.as_std_path(), b"@echo off\r\n").context("write stub")?;
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
