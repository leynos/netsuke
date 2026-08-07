//! Behavioural coverage for Windows `PATHEXT` handling through the public
//! `which` and `command_available` helpers.
//!
//! The unit tests in `src/stdlib/which/pathext_tests.rs` pin the normalization
//! rules by calling `parse_pathext` directly; these drive the same rules end to
//! end — template render, resolver, filesystem probe — so a value that
//! normalizes correctly but is never consulted would still fail here.
//!
//! `PATHEXT` arrives through `StdlibConfig::with_pathext_override`, so no test
//! touches the process environment.
//!
//! Be aware of the reach of this file: `PATHEXT` governs resolution only on
//! Windows, and `.github/workflows/ci.yml` runs `make test` on
//! `ubuntu-latest` alone, so nothing here executes on a merge today. It gates
//! for contributors developing on Windows, and would gate for everyone were a
//! Windows job added. The rules that must hold on every host — normalization
//! and the fallback — stay covered on Linux by
//! `src/stdlib/which/pathext_tests.rs`, which is why those tests call
//! `parse_pathext` directly rather than being folded into this suite.
#![cfg(windows)]

use std::ffi::OsString;

use anyhow::{Context, Result, anyhow, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, context};
use netsuke::stdlib::{self, StdlibConfig};
use rstest::{fixture, rstest};

/// A temporary workspace holding a `bin` directory of stub executables.
struct ToolWorkspace {
    _temp: tempfile::TempDir,
    root: Utf8PathBuf,
    bin: Utf8PathBuf,
}

impl ToolWorkspace {
    /// Create a stub executable named `filename` in `bin`.
    ///
    /// The extension is part of the name rather than a separate argument: it
    /// is what each test is choosing, so splitting the two would invite a
    /// caller to pass a stem that already carries one.
    fn write_tool(&self, filename: &str) -> Result<Utf8PathBuf> {
        let path = self.bin.join(filename);
        std::fs::write(path.as_std_path(), b"@echo off\r\n")
            .with_context(|| format!("write stub tool {path}"))?;
        Ok(path)
    }
}

#[fixture]
fn tool_workspace() -> Result<ToolWorkspace> {
    let temp = tempfile::tempdir().context("create temp workspace")?;
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("temp path should be UTF-8: {path:?}"))?;
    let bin = root.join("bin");
    std::fs::create_dir(bin.as_std_path()).with_context(|| format!("create {bin}"))?;
    Ok(ToolWorkspace {
        _temp: temp,
        root,
        bin,
    })
}

/// Register the stdlib with `PATH` and `PATHEXT` both pinned by configuration.
fn stdlib_env(workspace: &ToolWorkspace, pathext: &str) -> Result<Environment<'static>> {
    let dir = Dir::open_ambient_dir(&workspace.root, ambient_authority())
        .with_context(|| format!("open workspace {}", workspace.root))?;
    let config = StdlibConfig::new(dir)?
        .with_workspace_root_path(workspace.root.clone())?
        .with_path_override(OsString::from(workspace.bin.as_str()))
        .with_pathext_override(OsString::from(pathext));
    let mut env = Environment::new();
    stdlib::register_with_config(&mut env, config)?;
    Ok(env)
}

/// `which` renders paths with forward slashes on every platform.
fn rendered_form(path: &Utf8Path) -> String {
    path.as_str().replace('\\', "/")
}

fn assert_which_resolves_to(
    env: &Environment<'_>,
    command: &str,
    expected: &Utf8Path,
) -> Result<()> {
    let rendered = env.render_str(&format!("{{{{ which('{command}') }}}}"), context! {})?;
    let expected = rendered_form(expected);
    ensure!(
        rendered == expected,
        "expected which('{command}') to render {expected}, got {rendered}"
    );
    Ok(())
}

fn assert_command_unavailable(env: &Environment<'_>, command: &str) -> Result<()> {
    let rendered = env.render_str(
        &format!("{{{{ command_available('{command}') }}}}"),
        context! {},
    )?;
    ensure!(
        rendered == "false",
        "expected command_available('{command}') to be false, got {rendered}"
    );
    Ok(())
}

/// A blank `PATHEXT` must resolve as though it were unset.
///
/// Taking the value literally would leave no candidate extensions, so every
/// command would be reported missing. `.cmd` comes from the built-in list, so
/// resolving `helper` here can only be the fallback at work.
#[rstest]
#[case::whitespace_only("   ")]
#[case::separators_only(";;")]
#[case::whitespace_segments(" ; ; ")]
fn blank_pathext_falls_back_to_the_default_list(
    #[case] pathext: &str,
    tool_workspace: Result<ToolWorkspace>,
) -> Result<()> {
    let workspace = tool_workspace?;
    let tool = workspace.write_tool("helper.cmd")?;

    let env = stdlib_env(&workspace, pathext)?;

    assert_which_resolves_to(&env, "helper", &tool)
}

/// A `PATHEXT` written with stray spacing and mixed case is normalized, and the
/// normalized list is the one actually searched.
///
/// `.exe` resolves despite being written as ` .exe `, while `.cmd` — present in
/// the built-in list but absent from this value — does not, which is what
/// distinguishes the injected list from a silent fallback to the default.
#[rstest]
fn normalized_pathext_governs_resolution(tool_workspace: Result<ToolWorkspace>) -> Result<()> {
    let workspace = tool_workspace?;
    let executable = workspace.write_tool("helper.exe")?;
    workspace.write_tool("scripted.cmd")?;

    let env = stdlib_env(&workspace, " .COM ; .exe ")?;

    assert_which_resolves_to(&env, "helper", &executable)?;
    assert_command_unavailable(&env, "scripted")
}
