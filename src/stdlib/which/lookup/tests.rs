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

fn params<'a>(
    collect_all: bool,
    skip_dirs: &'a WorkspaceSkipList,
) -> WorkspaceSearchParams<'a> {
    WorkspaceSearchParams {
        collect_all,
        skip_dirs,
    }
}

/// Helper to execute workspace search with platform-specific env handling.
fn execute_workspace_search(
    workspace: &TempWorkspace,
    collect_all: bool,
    skip_dirs: &WorkspaceSkipList,
) -> Result<Vec<Utf8PathBuf>> {
    let snapshot =
        EnvSnapshot::capture(Some(workspace.root())).expect("capture env for workspace search");
    search_workspace(&snapshot, "tool", params(collect_all, skip_dirs))
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
    let skip_dirs = [".git", "node_modules", ".idea", ".vscode", "dist", "build"];
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
fn search_workspace_skips_directories_case_insensitively(
    workspace: TempWorkspace,
) -> Result<()> {
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

#[cfg(unix)]
#[rstest]
fn path_with_invalid_utf8_triggers_args_error(workspace: TempWorkspace) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;

    let invalid_path = std::ffi::OsStr::from_bytes(b"/bin:\xFF");
    let _guard = VarGuard::set("PATH", invalid_path);

    let err = EnvSnapshot::capture(Some(workspace.root()))
        .expect_err("invalid PATH should fail EnvSnapshot::capture");
    let msg = err.to_string();

    ensure!(
        msg.contains("netsuke::jinja::which::args"),
        "expected PATH parsing error, got: {msg}"
    );
    ensure!(
        msg.contains("PATH entry #1"),
        "expected PATH entry index in message, got: {msg}"
    );

    Ok(())
}

#[rstest]
fn relative_path_entries_resolve_against_cwd(workspace: TempWorkspace) -> Result<()> {
    let bin = workspace.root().join("bin");
    let tools = workspace.root().join("tools");
    fs::create_dir_all(bin.as_std_path()).context("mkdir bin")?;
    fs::create_dir_all(tools.as_std_path()).context("mkdir tools")?;

    let path_value = format!("{}:bin:tools", workspace.root());
    let _guard = VarGuard::set("PATH", std::ffi::OsStr::new(&path_value));

    let snapshot = EnvSnapshot::capture(Some(workspace.root()))
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
fn pathext_empty_uses_default_fallback(workspace: TempWorkspace) -> Result<()> {
    let _path_guard = VarGuard::set("PATH", std::ffi::OsStr::new(workspace.root().as_str()));
    let _pathext_guard = VarGuard::set("PATHEXT", std::ffi::OsStr::new(""));

    let snapshot =
        EnvSnapshot::capture(Some(workspace.root())).context("capture env for empty PATHEXT")?;
    let pathexts = snapshot.pathext();

    assert!(
        pathexts.iter().any(|ext| ext.eq_ignore_ascii_case(".com")),
        "default PATHEXT should include .COM",
    );
    assert!(
        pathexts.iter().any(|ext| ext.eq_ignore_ascii_case(".exe")),
        "default PATHEXT should include .EXE",
    );

    Ok(())
}

#[cfg(windows)]
#[rstest]
fn pathext_without_leading_dots_is_normalised_and_deduplicated(
    workspace: TempWorkspace,
) -> Result<()> {
    let _path_guard = VarGuard::set("PATH", std::ffi::OsStr::new(workspace.root().as_str()));
    let _pathext_guard = VarGuard::set("PATHEXT", std::ffi::OsStr::new("COM;EXE;EXE; .BAT ;bat"));

    let snapshot = EnvSnapshot::capture(Some(workspace.root()))?;
    let mut pathexts = snapshot.pathext().to_vec();
    pathexts.sort_unstable_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

    let contains_ci = |needle: &str| pathexts.iter().any(|ext| ext.eq_ignore_ascii_case(needle));

    assert!(contains_ci(".COM"));
    assert!(contains_ci(".EXE"));
    assert!(contains_ci(".BAT"));

    let mut lower: Vec<String> = pathexts
        .iter()
        .map(|ext| ext.to_ascii_lowercase())
        .collect();
    lower.sort_unstable();
    lower.dedup();
    assert_eq!(lower.len(), pathexts.len());

    Ok(())
}

#[cfg(unix)]
#[rstest]
fn direct_path_not_executable_raises_direct_not_found(workspace: TempWorkspace) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let script = workspace.root().join("script.sh");
    fs::write(script.as_std_path(), "#!/bin/sh\necho test\n").context("write script")?;
    let mut perms = fs::metadata(script.as_std_path())
        .context("stat script")?
        .permissions();
    perms.set_mode(0o644);
    fs::set_permissions(script.as_std_path(), perms).context("chmod script")?;

    let snapshot =
        EnvSnapshot::capture(Some(workspace.root())).context("capture env for direct path")?;

    let err = resolve_direct(script.as_str(), &snapshot, &WhichOptions::default())
        .expect_err("non-executable direct path should fail");
    let msg = err.to_string();

    ensure!(
        msg.contains("not executable"),
        "expected not executable message: {msg}"
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
