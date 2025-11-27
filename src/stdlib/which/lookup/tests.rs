//! Tests for the which lookup helpers, covering PATH search, workspace
//! fallback, canonicalisation, and platform-specific PATHEXT behaviour.

use super::*;
use anyhow::{Context, Result, anyhow, ensure};
use rstest::{fixture, rstest};
use std::fs;
use tempfile::TempDir;
#[cfg(windows)]
use test_support::make_executable;
use test_support::{env::VarGuard, write_exec};

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

/// Helper to execute workspace search with platform-specific env handling.
fn execute_workspace_search(
    workspace: &TempWorkspace,
    collect_all: bool,
    skip_dirs: &WorkspaceSkipList,
) -> Result<Vec<Utf8PathBuf>> {
    #[cfg(windows)]
    let snapshot =
        EnvSnapshot::capture(Some(workspace.root())).expect("capture env for workspace search");

    #[cfg(windows)]
    let results = search_workspace(
        workspace.root(),
        "tool",
        WorkspaceSearchParams {
            collect_all,
            skip_dirs,
        },
        &snapshot,
    )?;

    #[cfg(not(windows))]
    let results = search_workspace(
        workspace.root(),
        "tool",
        WorkspaceSearchParams {
            collect_all,
            skip_dirs,
        },
        (),
    )?;

    Ok(results)
}

/// Helper to test that workspace search skips a specific directory.
fn test_workspace_skips_directory(
    workspace: &TempWorkspace,
    dir_name: &str,
    skips: &WorkspaceSkipList,
    assertion_msg: &str,
) -> Result<()> {
    let heavy = workspace.root().join(dir_name);
    fs::create_dir_all(heavy.as_std_path()).with_context(|| format!("mkdir {dir_name}"))?;
    write_exec(heavy.as_path(), "tool")?;
    let results = execute_workspace_search(workspace, false, skips)?;
    ensure!(results.is_empty(), "{}", assertion_msg);
    Ok(())
}

#[rstest]
fn search_workspace_returns_executable_and_skips_non_exec(workspace: TempWorkspace) -> Result<()> {
    let exec = write_exec(workspace.root(), "tool")?;
    let non_exec = workspace.root().join("tool2");
    fs::write(non_exec.as_std_path(), b"not exec").context("write non exec")?;
    let skips = WorkspaceSkipList::default();

    let results = execute_workspace_search(&workspace, false, &skips)?;
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
    let skips = WorkspaceSkipList::default();

    let mut results = execute_workspace_search(&workspace, true, &skips)?;
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
    let skips = WorkspaceSkipList::default();
    test_workspace_skips_directory(
        &workspace,
        "target",
        &skips,
        "expected target/ to be skipped",
    )
}

#[rstest]
fn search_workspace_skips_common_editor_directories(workspace: TempWorkspace) -> Result<()> {
    let skip_dirs = [".git", "node_modules", ".idea", ".vscode"];
    for dir in skip_dirs {
        let path = workspace.root().join(dir);
        fs::create_dir_all(path.as_std_path()).context("mkdir skip dir")?;
        write_exec(path.as_path(), "tool")?;
    }
    let skips = WorkspaceSkipList::default();

    let results = execute_workspace_search(&workspace, false, &skips)?;
    ensure!(results.is_empty(), "expected editor caches to be skipped");
    Ok(())
}

#[cfg(windows)]
#[rstest]
fn search_workspace_skips_directories_case_insensitively(workspace: TempWorkspace) -> Result<()> {
    let skips = WorkspaceSkipList::default();
    test_workspace_skips_directory(
        &workspace,
        "TARGET",
        &skips,
        "expected TARGET/ to be skipped case-insensitively",
    )
}

#[rstest]
fn search_workspace_uses_custom_skip_configuration(workspace: TempWorkspace) -> Result<()> {
    let target = workspace.root().join("target");
    fs::create_dir_all(target.as_std_path()).context("mkdir target")?;
    let exec = write_exec(target.as_path(), "tool")?;
    let skips = WorkspaceSkipList::from_names([".git"]);

    let results = execute_workspace_search(&workspace, false, &skips)?;
    ensure!(
        results == vec![exec],
        "expected custom skip list to allow target/"
    );
    Ok(())
}

#[rstest]
fn lookup_respects_workspace_skip_configuration(workspace: TempWorkspace) -> Result<()> {
    let _guard = VarGuard::unset("PATH");

    let target = workspace.root().join("target");
    fs::create_dir_all(target.as_std_path()).context("mkdir target")?;
    let exec = write_exec(target.as_path(), "tool")?;
    let options = WhichOptions::default();

    let env_default = EnvSnapshot::capture(Some(workspace.root())).expect("capture env");
    let default_skips = WorkspaceSkipList::default();
    let err = lookup("tool", &env_default, &options, &default_skips)
        .expect_err("default skips should ignore target");
    ensure!(
        matches!(err.kind(), minijinja::ErrorKind::InvalidOperation),
        "expected not_found error"
    );

    let env_custom = EnvSnapshot::capture(Some(workspace.root())).expect("capture env");
    let custom_skips = WorkspaceSkipList::from_names([".git"]);
    let results = lookup("tool", &env_custom, &options, &custom_skips)?;
    ensure!(
        results == vec![exec],
        "expected discovery when target allowed"
    );

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
    let skips = WorkspaceSkipList::default();
    let results = execute_workspace_search(&workspace, false, &skips)?;
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
