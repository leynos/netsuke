//! Snapshot tests for user-visible `which` diagnostics.

use std::ffi::OsString;

use anyhow::{Context, Result, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, context};
use netsuke::stdlib::{self, StdlibConfig};
use rstest::{fixture, rstest};

struct SnapshotWorkspace {
    _temp: tempfile::TempDir,
    root: Utf8PathBuf,
}

#[fixture]
fn snapshot_workspace() -> Result<SnapshotWorkspace> {
    let temp = tempfile::tempdir().context("create snapshot workspace")?;
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("temp path should be UTF-8: {path:?}"))?;
    Ok(SnapshotWorkspace { _temp: temp, root })
}

fn stdlib_env(root: &Utf8Path) -> Result<Environment<'static>> {
    let workspace = Dir::open_ambient_dir(root, ambient_authority())
        .with_context(|| format!("open workspace {root}"))?;
    let config = StdlibConfig::new(workspace)?
        .with_workspace_root_path(root.to_path_buf())?
        .with_path_override(OsString::new());
    let mut env = Environment::new();
    stdlib::register_with_config(&mut env, config)?;
    Ok(env)
}

fn write_tool(root: &Utf8Path) -> Result<()> {
    let path = root.join(tool_filename("tool"));
    std::fs::write(path.as_std_path(), script_contents())
        .with_context(|| format!("write fixture tool {path}"))?;
    mark_executable(&path)
}

fn tool_filename(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.cmd")
    } else {
        name.to_owned()
    }
}

const fn script_contents() -> &'static [u8] {
    #[cfg(windows)]
    {
        b"@echo off\r\n"
    }
    #[cfg(not(windows))]
    {
        b"#!/bin/sh\nexit 0\n"
    }
}

#[cfg(unix)]
fn mark_executable(path: &Utf8Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path.as_std_path())
        .with_context(|| format!("stat {path}"))?
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path.as_std_path(), perms).with_context(|| format!("chmod {path}"))
}

#[cfg(not(unix))]
fn mark_executable(_path: &Utf8Path) -> Result<()> {
    Ok(())
}

fn render_error(env: &Environment<'_>, template: &str) -> Result<String> {
    match env.render_str(template, context! {}) {
        Ok(output) => anyhow::bail!("template should fail, got {output}"),
        Err(err) => Ok(err.to_string()),
    }
}

fn normalize_error(message: &str, root: &Utf8Path) -> String {
    message.replace(root.as_str(), "[WORKSPACE]")
}

#[rstest]
#[case::not_found("which_not_found", "{{ 'absent' | which(cwd_mode='never') }}")]
#[case::direct_not_found("which_direct_not_found", "{{ './absent' | which }}")]
#[case::empty_command("which_args_empty_command", "{{ '' | which }}")]
#[case::invalid_cwd_mode(
    "which_args_invalid_cwd_mode",
    "{{ 'tool' | which(cwd_mode='invalid') }}"
)]
#[case::unknown_keyword("which_args_unknown_keyword", "{{ 'tool' | which(unexpected=true) }}")]
fn which_diagnostic_messages_match_baseline(
    #[case] snapshot: &str,
    #[case] template: &str,
    snapshot_workspace: Result<SnapshotWorkspace>,
) -> Result<()> {
    let workspace_fixture = snapshot_workspace?;
    write_tool(&workspace_fixture.root)?;
    let env = stdlib_env(&workspace_fixture.root)?;
    let message = render_error(&env, template)?;
    let normalized = normalize_error(&message, &workspace_fixture.root);

    insta::assert_snapshot!(snapshot, normalized);
    Ok(())
}
