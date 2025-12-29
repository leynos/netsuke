//! Shared fixtures for the `which` filter/function integration tests.

use anyhow::{Context, Result, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use minijinja::{context, Environment};
use std::ffi::{OsStr, OsString};

use test_support::{env::VarGuard, env_lock::EnvLock};

use super::support::{self, fallible};

#[derive(Debug, Clone)]
pub(crate) struct ToolName(String);

impl ToolName {
    pub(crate) fn new(name: impl Into<String>) -> Self { Self(name.into()) }
    pub(crate) fn as_str(&self) -> &str { &self.0 }
}

impl From<&str> for ToolName {
    fn from(s: &str) -> Self { Self(s.to_owned()) }
}

impl AsRef<str> for ToolName {
    fn as_ref(&self) -> &str { &self.0 }
}

#[derive(Debug, Clone)]
pub(crate) struct DirName(String);

impl DirName {
    pub(crate) fn new(name: impl Into<String>) -> Self { Self(name.into()) }
    pub(crate) fn as_str(&self) -> &str { &self.0 }
}

impl From<&str> for DirName {
    fn from(s: &str) -> Self { Self(s.to_owned()) }
}

impl AsRef<str> for DirName {
    fn as_ref(&self) -> &str { &self.0 }
}

impl AsRef<OsStr> for DirName {
    fn as_ref(&self) -> &OsStr { OsStr::new(&self.0) }
}

#[derive(Debug, Clone)]
pub(crate) struct Template(String);

impl Template {
    pub(crate) fn new(template: impl Into<String>) -> Self { Self(template.into()) }
    pub(crate) fn as_str(&self) -> &str { &self.0 }
}

impl From<&str> for Template {
    fn from(s: &str) -> Self { Self(s.to_owned()) }
}

impl AsRef<str> for Template {
    fn as_ref(&self) -> &str { &self.0 }
}

pub(crate) struct PathEnv {
    _lock: EnvLock,
    path_guard: VarGuard,
    #[cfg(windows)]
    pathext_guard: VarGuard,
}

impl PathEnv {
    pub(crate) fn new(entries: &[Utf8PathBuf]) -> Result<Self> {
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

pub(crate) fn write_tool(dir: &Utf8Path, name: &ToolName) -> Result<Utf8PathBuf> {
    let filename = tool_name(name);
    let path = dir.join(Utf8Path::new(&filename));
    let parent = path
        .parent()
        .context("tool path should have a parent directory")?;
    std::fs::create_dir_all(parent.as_std_path())
        .with_context(|| format!("create parent for {path:?}"))?;
    std::fs::write(path.as_std_path(), script_contents())
        .with_context(|| format!("write fixture {path:?}"))?;
    mark_executable(&path)?;
    Ok(path)
}

#[cfg(unix)]
fn mark_executable(path: &Utf8Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path.as_std_path())
        .with_context(|| format!("stat {path:?}"))?
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path.as_std_path(), perms)
        .with_context(|| format!("chmod {path:?}"))
}

#[cfg(not(unix))]
fn mark_executable(_path: &Utf8Path) -> Result<()> { Ok(()) }

#[cfg(windows)]
fn tool_name(base: &ToolName) -> String { format!("{}.cmd", base.as_str()) }

#[cfg(not(windows))]
fn tool_name(base: &ToolName) -> String { base.as_str().to_owned() }

fn script_contents() -> &'static [u8] {
    #[cfg(windows)]
    { b"@echo off\r\n" }
    #[cfg(not(windows))]
    { b"#!/bin/sh\nexit 0\n" }
}

pub(crate) fn render(env: &mut Environment<'_>, template: &Template) -> Result<String> {
    env.render_str(template.as_str(), context! {})
        .map_err(|err| anyhow!(err.to_string()))
}

pub(crate) struct WhichTestFixture {
    _temp: tempfile::TempDir,
    pub(crate) env: Environment<'static>,
    pub(crate) state: netsuke::stdlib::StdlibState,
    pub(crate) paths: Vec<Utf8PathBuf>,
    _path_env: PathEnv,
}

impl WhichTestFixture {
    pub(crate) fn with_tool_in_dirs(tool_name: &ToolName, dir_names: &[DirName]) -> Result<Self> {
        let (temp, root) = support::filter_workspace()?;
        let mut dirs = Vec::new();
        let mut tool_paths = Vec::new();
        for dir_name in dir_names {
            let dir = root.join(dir_name.as_str());
            std::fs::create_dir_all(dir.as_std_path())
                .with_context(|| format!("create directory {}", dir))?;
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

    pub(crate) fn render(&mut self, template: &Template) -> Result<String> {
        self.env
            .render_str(template.as_str(), context! {})
            .map_err(|err| anyhow!(err.to_string()))
    }
}
