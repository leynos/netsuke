//! Helpers for preparing stdlib workspaces during BDD scenarios, wiring
//! up temporary directories, fixtures, and environment overrides for tests.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::types::{FileContents, HelperName, HttpResponseBody, PathEntries};
use anyhow::{Context, Result, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest_bdd_macros::given;
use std::{env, ffi::OsStr, fs};
use test_support::{
    command_helper::{
        compile_failure_helper, compile_large_output_helper, compile_uppercase_helper,
    },
    env::set_var,
};

use super::types::TemplatePath;

const LINES_FIXTURE: &str = concat!("one\n", "two\n", "three\n",);

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

pub(crate) fn ensure_workspace(world: &TestWorld) -> Result<Utf8PathBuf> {
    if let Some(root) = world.stdlib_root.get() {
        return Ok(root);
    }
    let temp = tempfile::tempdir().context("create stdlib workspace")?;
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("stdlib workspace path is not valid UTF-8: {path:?}"))?;
    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open stdlib workspace directory")?;
    handle
        .write("file", b"data")
        .context("write stdlib file fixture")?;
    handle
        .write("lines.txt", LINES_FIXTURE.as_bytes())
        .context("write stdlib lines fixture")?;
    #[cfg(unix)]
    handle
        .symlink("file", "link")
        .context("create stdlib symlink fixture")?;
    #[cfg(not(unix))]
    handle
        .write("link", b"data")
        .context("write stdlib link fixture")?;
    world.temp_dir.set_value(temp);
    world.stdlib_root.set(root.clone());
    Ok(root)
}

/// Compile a command helper and register it in the world state.
fn compile_and_register_helper<F>(
    world: &TestWorld,
    helper_name: HelperName,
    compile_fn: F,
) -> Result<()>
where
    F: FnOnce(&Dir, &Utf8PathBuf, &str) -> Result<Utf8PathBuf>,
{
    let root = ensure_workspace(world)?;
    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open stdlib workspace directory")?;
    let name = helper_name.as_str();
    let helper =
        compile_fn(&handle, &root, name).with_context(|| format!("compile {name} helper"))?;
    world.stdlib_command.set(format!("\"{}\"", helper.as_str()));
    Ok(())
}

/// Start an HTTP server returning the given body content.
fn start_http_server(world: &TestWorld, body: HttpResponseBody) -> Result<()> {
    world.shutdown_http_server();
    let (url, server) = test_support::http::spawn_http_server(body.into_string())
        .context("spawn HTTP server for stdlib steps")?;
    world.stdlib_url.set(url);
    world.http_server.set_value(server);
    Ok(())
}

/// Write file contents to a path within the stdlib workspace.
fn write_file_to_workspace(
    world: &TestWorld,
    path: TemplatePath,
    contents: FileContents,
) -> Result<()> {
    let root = ensure_workspace(world)?;
    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open stdlib workspace directory")?;
    if let Some(parent) = path.as_path().parent().filter(|p| !p.as_str().is_empty()) {
        handle
            .create_dir_all(parent)
            .context("create stdlib fixture directories")?;
    }
    handle
        .write(path.as_path(), contents.as_bytes())
        .context("write stdlib fixture file")?;
    Ok(())
}

/// Create an executable script at the given path within the stdlib workspace.
fn create_executable(world: &TestWorld, path: TemplatePath) -> Result<()> {
    let root = ensure_workspace(world)?;
    let relative = Utf8PathBuf::from(path.as_str());
    let target = resolve_executable_path(&root, &relative);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent.as_std_path())
            .with_context(|| format!("create directories for stdlib executable at {parent}"))?;
    }
    fs::write(target.as_std_path(), executable_script())
        .with_context(|| format!("write stdlib executable {target}"))?;
    mark_executable(&target)?;
    Ok(())
}

/// Configure the PATH environment variable with the given entries.
fn configure_path_environment(world: &TestWorld, entries: PathEntries) -> Result<()> {
    let root = ensure_workspace(world)?;
    let trimmed = entries.as_str().trim();
    let dirs: Vec<Utf8PathBuf> = if trimmed.is_empty() {
        Vec::new()
    } else {
        trimmed
            .split(':')
            .filter(|segment| !segment.trim().is_empty())
            .map(|segment| root.join(segment.trim()))
            .collect()
    };
    for dir in &dirs {
        fs::create_dir_all(dir.as_std_path())
            .with_context(|| format!("create PATH directory {dir}"))?;
    }
    let joined = if dirs.is_empty() {
        std::ffi::OsString::new()
    } else {
        env::join_paths(dirs.iter().map(|dir| dir.as_std_path()))
            .context("join stdlib PATH entries")?
    };
    let previous = set_var("PATH", joined.as_os_str());
    world.track_env_var("PATH".into(), previous);
    #[cfg(windows)]
    {
        let previous = set_var("PATHEXT", OsStr::new(".cmd;.exe"));
        world.track_env_var("PATHEXT".into(), previous);
    }
    Ok(())
}

pub(crate) fn resolve_template_path(root: &Utf8Path, raw: TemplatePath) -> TemplatePath {
    if raw.as_str().starts_with('~') {
        return raw;
    }
    let candidate = raw.as_path();
    if candidate.is_absolute() {
        raw
    } else {
        TemplatePath::from(root.join(candidate))
    }
}

pub(super) fn resolve_executable_path(root: &Utf8Path, relative: &Utf8Path) -> Utf8PathBuf {
    #[cfg(windows)]
    let mut path = root.join(relative);
    #[cfg(not(windows))]
    let path = root.join(relative);
    #[cfg(windows)]
    {
        if path.extension().is_none() {
            path.set_extension("cmd");
        }
    }
    path
}

const fn executable_script() -> &'static [u8] {
    #[cfg(windows)]
    {
        b"@echo off\r\n"
    }
    #[cfg(not(windows))]
    {
        b"#!/bin/sh\nexit 0\n"
    }
}

fn mark_executable(path: &Utf8Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path.as_std_path())
            .with_context(|| format!("stat stdlib executable {path}"))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path.as_std_path(), perms)
            .with_context(|| format!("chmod stdlib executable {path}"))?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("a stdlib workspace")]
pub(crate) fn stdlib_workspace(world: &TestWorld) -> Result<()> {
    let root = ensure_workspace(world)?;
    world.stdlib_root.set(root);
    Ok(())
}

#[given("an uppercase stdlib command helper")]
pub(crate) fn uppercase_stdlib_command_helper(world: &TestWorld) -> Result<()> {
    compile_and_register_helper(
        world,
        HelperName::from("cmd_upper"),
        compile_uppercase_helper,
    )
}

#[given("a failing stdlib command helper")]
pub(crate) fn failing_stdlib_command_helper(world: &TestWorld) -> Result<()> {
    compile_and_register_helper(world, HelperName::from("cmd_fail"), compile_failure_helper)
}

#[given("a large-output stdlib command helper")]
pub(crate) fn large_output_stdlib_command_helper(world: &TestWorld) -> Result<()> {
    compile_and_register_helper(
        world,
        HelperName::from("cmd_large"),
        compile_large_output_helper,
    )
}

#[given("an HTTP server returning {body:string}")]
pub(crate) fn http_server_returning(world: &TestWorld, body: &str) -> Result<()> {
    start_http_server(world, HttpResponseBody::new(body))
}

#[given("the stdlib file {path:string} contains {contents:string}")]
pub(crate) fn write_stdlib_file(world: &TestWorld, path: &str, contents: &str) -> Result<()> {
    write_file_to_workspace(world, TemplatePath::new(path), FileContents::new(contents))
}

#[given("the stdlib executable {path:string} exists")]
pub(crate) fn stdlib_executable_exists(world: &TestWorld, path: &str) -> Result<()> {
    create_executable(world, TemplatePath::new(path))
}

#[given("the stdlib PATH entries are {entries:string}")]
pub(crate) fn stdlib_path_entries(world: &TestWorld, entries: &str) -> Result<()> {
    configure_path_environment(world, PathEntries::new(entries))
}

#[given("HOME points to the stdlib workspace root")]
pub(crate) fn home_points_to_stdlib_root(world: &TestWorld) -> Result<()> {
    let root = ensure_workspace(world)?;
    let os_root = OsStr::new(root.as_str());
    let previous = set_var("HOME", os_root);
    world.track_env_var("HOME".into(), previous);
    #[cfg(windows)]
    {
        let previous = set_var("USERPROFILE", os_root);
        world.track_env_var("USERPROFILE".into(), previous);
    }
    world.stdlib_root.set(root);
    Ok(())
}
