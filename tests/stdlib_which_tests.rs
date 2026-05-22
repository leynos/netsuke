//! Integration tests for `which` and `command_available` stdlib helpers.

use std::{env, ffi::OsString};

use anyhow::{Context, Result, anyhow, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, context};
use netsuke::stdlib::{self, StdlibConfig};
use rstest::{fixture, rstest};

struct StdlibWorkspace {
    _temp: tempfile::TempDir,
    root: Utf8PathBuf,
}

fn stdlib_env(root: &Utf8Path, path_override: OsString) -> Result<Environment<'static>> {
    let workspace = Dir::open_ambient_dir(root, ambient_authority())
        .with_context(|| format!("open workspace {root}"))?;
    let config = StdlibConfig::new(workspace)?
        .with_workspace_root_path(root.to_path_buf())?
        .with_path_override(path_override);
    let mut env = Environment::new();
    stdlib::register_with_config(&mut env, config)?;
    Ok(env)
}

#[fixture]
fn stdlib_workspace() -> Result<StdlibWorkspace> {
    let temp = tempfile::tempdir().context("create temp workspace")?;
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("temp path should be UTF-8: {path:?}"))?;
    Ok(StdlibWorkspace { _temp: temp, root })
}

fn env_without_path(workspace: &StdlibWorkspace) -> Result<Environment<'static>> {
    stdlib_env(&workspace.root, path_override(&[])?)
}

fn path_override(entries: &[Utf8PathBuf]) -> Result<OsString> {
    if entries.is_empty() {
        Ok(OsString::new())
    } else {
        env::join_paths(entries.iter().map(|entry| entry.as_std_path()))
            .context("join PATH entries")
    }
}

fn write_tool(dir: &Utf8Path, name: &str) -> Result<Utf8PathBuf> {
    let path = dir.join(tool_filename(name));
    std::fs::create_dir_all(dir.as_std_path()).with_context(|| format!("create {dir}"))?;
    std::fs::write(path.as_std_path(), script_contents())
        .with_context(|| format!("write fixture tool {path}"))?;
    mark_executable(&path)?;
    Ok(path)
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

#[rstest]
fn command_available_returns_true_for_path_match(
    stdlib_workspace: Result<StdlibWorkspace>,
) -> Result<()> {
    let workspace_fixture = stdlib_workspace?;
    let bin = workspace_fixture.root.join("bin");
    write_tool(&bin, "helper")?;
    let env = stdlib_env(&workspace_fixture.root, path_override(&[bin])?)?;

    let output = env.render_str("{{ command_available('helper') }}", context! {})?;

    ensure!(output == "true", "expected true, got {output}");
    Ok(())
}

#[rstest]
fn command_available_returns_false_for_missing_command(
    stdlib_workspace: Result<StdlibWorkspace>,
) -> Result<()> {
    let workspace_fixture = stdlib_workspace?;
    let env = env_without_path(&workspace_fixture)?;

    let output = env.render_str(
        "{{ command_available('absent', cwd_mode='never') }}",
        context! {},
    )?;

    ensure!(output == "false", "expected false, got {output}");
    Ok(())
}

fn assert_render_error_contains(
    env: &Environment<'_>,
    template: &str,
    context_msg: &str,
    expected_fragment: &str,
) -> Result<()> {
    let err = env
        .render_str(template, context! {})
        .err()
        .with_context(|| context_msg.to_owned())?;
    let message = err.to_string();
    ensure!(
        message.contains(expected_fragment),
        "expected {expected_fragment:?} in error message, got {message}"
    );
    Ok(())
}

#[rstest]
fn command_available_rejects_empty_command(
    stdlib_workspace: Result<StdlibWorkspace>,
) -> Result<()> {
    let workspace_fixture = stdlib_workspace?;
    let env = env_without_path(&workspace_fixture)?;
    assert_render_error_contains(
        &env,
        "{{ command_available('') }}",
        "empty command should fail",
        "netsuke::jinja::which::args",
    )
}

#[rstest]
fn command_available_rejects_unknown_keyword(
    stdlib_workspace: Result<StdlibWorkspace>,
) -> Result<()> {
    let workspace_fixture = stdlib_workspace?;
    let env = env_without_path(&workspace_fixture)?;
    assert_render_error_contains(
        &env,
        "{{ command_available('absent', unexpected=true) }}",
        "unknown keyword should fail",
        "unknown keyword argument",
    )
}

#[rstest]
fn which_filter_reports_missing_command(stdlib_workspace: Result<StdlibWorkspace>) -> Result<()> {
    let workspace_fixture = stdlib_workspace?;
    let env = env_without_path(&workspace_fixture)?;
    assert_render_error_contains(
        &env,
        "{{ 'absent' | which }}",
        "render should fail for missing command",
        "netsuke::jinja::which::not_found",
    )
}
