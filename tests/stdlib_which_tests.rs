//! Integration tests for `which` and `command_available` stdlib helpers.

use std::{env, ffi::OsString};

use anyhow::{Context, Result, anyhow, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, context};
use netsuke::stdlib::{self, StdlibConfig};
use rstest::{fixture, rstest};
use test_support::{env::VarGuard, env_lock::EnvLock};

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

fn stdlib_env_from_process(root: &Utf8Path) -> Result<Environment<'static>> {
    let workspace = Dir::open_ambient_dir(root, ambient_authority())
        .with_context(|| format!("open workspace {root}"))?;
    let config = StdlibConfig::new(workspace)?.with_workspace_root_path(root.to_path_buf())?;
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

fn render_command_available(
    env: &Environment<'_>,
    command: &Utf8Path,
    kwargs: &str,
) -> Result<String> {
    let template = format!(
        "{{{{ command_available({command:?}{kwargs}) }}}}",
        command = command.as_str()
    );
    env.render_str(&template, context! {})
        .map_err(anyhow::Error::from)
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

#[rstest]
fn command_available_returns_false_for_missing_absolute_path(
    stdlib_workspace: Result<StdlibWorkspace>,
) -> Result<()> {
    let workspace_fixture = stdlib_workspace?;
    let env = env_without_path(&workspace_fixture)?;
    let missing = workspace_fixture.root.join(tool_filename("missing"));

    let output = render_command_available(&env, missing.as_path(), "")?;

    ensure!(output == "false", "expected false, got {output}");
    Ok(())
}

#[rstest]
fn command_available_returns_false_for_missing_relative_path(
    stdlib_workspace: Result<StdlibWorkspace>,
) -> Result<()> {
    let workspace_fixture = stdlib_workspace?;
    let env = env_without_path(&workspace_fixture)?;

    let output = env.render_str("{{ command_available('./missing') }}", context! {})?;

    ensure!(output == "false", "expected false, got {output}");
    Ok(())
}

#[rstest]
fn command_available_returns_true_for_absolute_path(
    stdlib_workspace: Result<StdlibWorkspace>,
) -> Result<()> {
    let workspace_fixture = stdlib_workspace?;
    let tool = write_tool(&workspace_fixture.root.join("bin"), "absolute-helper")?;
    let env = env_without_path(&workspace_fixture)?;

    let output = render_command_available(&env, tool.as_path(), "")?;

    ensure!(output == "true", "expected true, got {output}");
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn command_available_returns_true_for_canonical_symlink(
    stdlib_workspace: Result<StdlibWorkspace>,
) -> Result<()> {
    let workspace_fixture = stdlib_workspace?;
    let tool = write_tool(&workspace_fixture.root.join("bin"), "canonical-helper")?;
    let link = workspace_fixture.root.join("bin").join("canonical-link");
    std::os::unix::fs::symlink(tool.as_std_path(), link.as_std_path())
        .with_context(|| format!("symlink {link} -> {tool}"))?;
    let env = env_without_path(&workspace_fixture)?;

    let output = render_command_available(&env, link.as_path(), ", canonical=true")?;

    ensure!(output == "true", "expected true, got {output}");
    Ok(())
}

#[rstest]
#[case::present("workspace-helper", "true")]
#[case::absent("missing-helper", "false")]
fn command_available_uses_workspace_fallback_when_path_is_empty(
    #[case] command: &str,
    #[case] expected: &str,
    stdlib_workspace: Result<StdlibWorkspace>,
) -> Result<()> {
    let workspace_fixture = stdlib_workspace?;
    write_tool(&workspace_fixture.root, "workspace-helper")?;
    let env = env_without_path(&workspace_fixture)?;
    let template = format!("{{{{ command_available({command:?}) }}}}");

    let output = env.render_str(&template, context! {})?;

    ensure!(output == expected, "expected {expected}, got {output}");
    Ok(())
}

#[rstest]
fn command_available_fresh_bypasses_cached_success(
    stdlib_workspace: Result<StdlibWorkspace>,
) -> Result<()> {
    let _lock = EnvLock::acquire();
    let workspace_fixture = stdlib_workspace?;
    let bin = workspace_fixture.root.join("bin");
    let tool = write_tool(&bin, "cached-helper")?;
    let path = path_override(std::slice::from_ref(&bin))?;
    let _path_guard = VarGuard::set("PATH", path.as_os_str());
    let env = stdlib_env_from_process(&workspace_fixture.root)?;

    let first = env.render_str("{{ command_available('cached-helper') }}", context! {})?;
    std::fs::remove_file(tool.as_std_path()).with_context(|| format!("remove {tool}"))?;
    let cached = env.render_str("{{ command_available('cached-helper') }}", context! {})?;
    let fresh = env.render_str(
        "{{ command_available('cached-helper', fresh=true) }}",
        context! {},
    )?;

    ensure!(first == "true", "expected initial true, got {first}");
    ensure!(cached == "true", "expected cached true, got {cached}");
    ensure!(fresh == "false", "expected fresh false, got {fresh}");
    Ok(())
}

#[rstest]
#[case::auto_present("auto", true, "true")]
#[case::auto_absent("auto", false, "false")]
#[case::always_present("always", true, "true")]
#[case::always_absent("always", false, "false")]
#[case::never_present("never", true, "false")]
#[case::never_absent("never", false, "false")]
fn command_available_honours_cwd_mode(
    #[case] cwd_mode: &str,
    #[case] present: bool,
    #[case] expected: &str,
    stdlib_workspace: Result<StdlibWorkspace>,
) -> Result<()> {
    let workspace_fixture = stdlib_workspace?;
    if present {
        write_tool(&workspace_fixture.root, "cwd-helper")?;
    }
    let env = env_without_path(&workspace_fixture)?;
    let template = format!("{{{{ command_available('cwd-helper', cwd_mode={cwd_mode:?}) }}}}");

    let output = env.render_str(&template, context! {})?;

    ensure!(output == expected, "expected {expected}, got {output}");
    Ok(())
}

#[rstest]
#[case::present(false, "true")]
#[case::present_all(true, "true")]
#[case::absent(false, "false")]
#[case::absent_all(true, "false")]
fn command_available_all_kwarg_does_not_affect_bool(
    #[case] all: bool,
    #[case] expected: &str,
    stdlib_workspace: Result<StdlibWorkspace>,
) -> Result<()> {
    let workspace_fixture = stdlib_workspace?;
    write_tool(&workspace_fixture.root.join("bin"), "all-helper")?;
    let env = stdlib_env(
        &workspace_fixture.root,
        path_override(&[workspace_fixture.root.join("bin")])?,
    )?;
    let command = if expected == "true" {
        "all-helper"
    } else {
        "missing-helper"
    };
    let template = format!("{{{{ command_available({command:?}, all={all}) }}}}");

    let output = env.render_str(&template, context! {})?;

    ensure!(output == expected, "expected {expected}, got {output}");
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
#[case::empty("{{ command_available('') }}", "netsuke::jinja::which::args")]
#[case::blank("{{ command_available('   ') }}", "netsuke::jinja::which::args")]
#[case::invalid_cwd_mode(
    "{{ command_available('tool', cwd_mode='invalid') }}",
    "netsuke::jinja::which::args"
)]
#[case::unknown_keyword(
    "{{ command_available('tool', unexpected=true) }}",
    "unknown keyword argument"
)]
fn command_available_rejects_invalid_arguments(
    #[case] template: &str,
    #[case] expected_fragment: &str,
    stdlib_workspace: Result<StdlibWorkspace>,
) -> Result<()> {
    let workspace_fixture = stdlib_workspace?;
    let env = env_without_path(&workspace_fixture)?;
    assert_render_error_contains(
        &env,
        template,
        "invalid command_available arguments should fail",
        expected_fragment,
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
