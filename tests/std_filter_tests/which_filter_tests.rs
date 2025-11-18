use anyhow::{Context, Result, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use minijinja::{context, Environment};
use rstest::rstest;
use std::ffi::{OsStr, OsString};
use test_support::{env::VarGuard, env_lock::EnvLock};

use super::support::{self, fallible};

struct PathEnv {
    _lock: EnvLock,
    path_guard: VarGuard,
    #[cfg(windows)]
    pathext_guard: VarGuard,
}

impl PathEnv {
    fn new(entries: &[Utf8PathBuf]) -> Result<Self> {
        let lock = EnvLock::acquire();
        let joined = if entries.is_empty() {
            OsString::new()
        } else {
            std::env::join_paths(entries.iter().map(|entry| entry.as_std_path()))
                .context("join PATH entries")?
        };
        let path_guard = VarGuard::set("PATH", joined.as_os_str());
        #[cfg(windows)]
        let pathext_guard = VarGuard::set("PATHEXT", OsStr::new(".cmd;.exe"));
        Ok(Self {
            _lock: lock,
            path_guard,
            #[cfg(windows)]
            pathext_guard,
        })
    }
}

fn write_tool(dir: &Utf8Path, name: &str) -> Result<Utf8PathBuf> {
    let filename = tool_name(name);
    let path = dir.join(Utf8Path::new(&filename));
    let parent = path
        .parent()
        .context("tool path should have a parent directory")?;
    std::fs::create_dir_all(parent.as_std_path())
        .with_context(|| format!("create parent for {:?}", path))?;
    std::fs::write(path.as_std_path(), script_contents())
        .with_context(|| format!("write fixture {:?}", path))?;
    mark_executable(&path)?;
    Ok(path)
}

#[cfg(unix)]
fn mark_executable(path: &Utf8Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path.as_std_path())
        .with_context(|| format!("stat {:?}", path))?
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path.as_std_path(), perms)
        .with_context(|| format!("chmod {:?}", path))
}

#[cfg(not(unix))]
fn mark_executable(_path: &Utf8Path) -> Result<()> {
    Ok(())
}

#[cfg(windows)]
fn tool_name(base: &str) -> String {
    format!("{base}.cmd")
}

#[cfg(not(windows))]
fn tool_name(base: &str) -> String {
    base.to_owned()
}

fn script_contents() -> &'static [u8] {
    #[cfg(windows)]
    {
        b"@echo off\r\n"
    }
    #[cfg(not(windows))]
    {
        b"#!/bin/sh\nexit 0\n"
    }
}

fn render(env: &mut Environment<'_>, template: &str) -> Result<String> {
    env.render_str(template, context! {})
        .map_err(|err| anyhow!(err.to_string()))
}

struct WhichTestFixture {
    _temp: tempfile::TempDir,
    env: Environment<'static>,
    state: netsuke::stdlib::StdlibState,
    paths: Vec<Utf8PathBuf>,
    _path_env: PathEnv,
}

impl WhichTestFixture {
    fn with_tool_in_dirs(tool_name: &str, dir_names: &[&str]) -> Result<Self> {
        let (temp, root) = support::filter_workspace()?;
        let mut dirs = Vec::new();
        let mut tool_paths = Vec::new();
        for dir_name in dir_names {
            let dir = root.join(dir_name);
            std::fs::create_dir_all(dir.as_std_path())?;
            let tool_path = write_tool(&dir, tool_name)?;
            dirs.push(dir);
            tool_paths.push(tool_path);
        }
        let path_env = PathEnv::new(&dirs)?;
        let (env, state) = fallible::stdlib_env_with_state()?;
        Ok(Self {
            _temp: temp,
            env,
            state,
            paths: tool_paths,
            _path_env: path_env,
        })
    }

    fn render(&mut self, template: &str) -> Result<String> {
        self.env
            .render_str(template, context! {})
            .map_err(|err| anyhow!(err.to_string()))
    }
}

#[rstest]
fn which_filter_returns_first_match() -> Result<()> {
    let mut fixture =
        WhichTestFixture::with_tool_in_dirs("helper", &["bin_first", "bin_second"])?;
    fixture.state.reset_impure();
    let output = fixture.render("{{ 'helper' | which }}")?;
    assert_eq!(output, fixture.paths[0].as_str());
    assert!(!fixture.state.is_impure());
    Ok(())
}

#[rstest]
fn which_filter_all_returns_all_matches() -> Result<()> {
    let mut fixture = WhichTestFixture::with_tool_in_dirs("helper", &["bin_a", "bin_b"])?;
    let output = fixture.render("{{ 'helper' | which(all=true) | join('|') }}")?;
    let expected = format!(
        "{}|{}",
        fixture.paths[0].as_str(),
        fixture.paths[1].as_str()
    );
    assert_eq!(output, expected);
    Ok(())
}

#[rstest]
fn which_function_honours_cwd_mode() -> Result<()> {
    let (_temp, root) = support::filter_workspace()?;
    let tool = write_tool(&root, "local")?;
    let _path = PathEnv::new(&[])?;
    let (mut env, _state) = fallible::stdlib_env_with_state()?;
    let template = "{{ which('local', cwd_mode='always') }}";
    let output = render(&mut env, template)?;
    assert_eq!(output, tool.as_str());
    Ok(())
}

#[rstest]
fn which_filter_reports_missing_command() -> Result<()> {
    let (_temp, _root) = support::filter_workspace()?;
    let _path = PathEnv::new(&[])?;
    let (mut env, _state) = fallible::stdlib_env_with_state()?;
    let err = env
        .render_str("{{ 'absent' | which }}", context! {})
        .unwrap_err();
    let message = err.to_string();
    assert!(message.contains("netsuke::jinja::which::not_found"));
    Ok(())
}
