//! Helpers for preparing stdlib workspaces during Cucumber scenarios, wiring
//! up temporary directories, fixtures, and environment overrides for tests.
use crate::CliWorld;
use anyhow::{Context, Result, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use cucumber::given;
use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
};
use test_support::{
    command_helper::{
        compile_failure_helper, compile_large_output_helper, compile_uppercase_helper,
    },
    env::set_var,
};

use super::types::{FileContent, PathEntries, RelativePath, ServerBody, TemplatePath};

const LINES_FIXTURE: &str = concat!("one\n", "two\n", "three\n",);

pub(crate) fn ensure_workspace(world: &mut CliWorld) -> Result<Utf8PathBuf> {
    if let Some(root) = &world.stdlib_root {
        return Ok(root.clone());
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
    world.temp = Some(temp);
    world.stdlib_root = Some(root.clone());
    Ok(root)
}

#[given("a stdlib workspace")]
pub(crate) fn stdlib_workspace(world: &mut CliWorld) -> Result<()> {
    let root = ensure_workspace(world)?;
    world.stdlib_root = Some(root);
    Ok(())
}

#[given("an uppercase stdlib command helper")]
pub(crate) fn uppercase_stdlib_command_helper(world: &mut CliWorld) -> Result<()> {
    let root = ensure_workspace(world)?;
    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open stdlib workspace directory")?;
    let helper = compile_uppercase_helper(&handle, &root, "cmd_upper")
        .context("compile uppercase helper")?;
    world.stdlib_command = Some(format!("\"{}\"", helper.as_str()));
    Ok(())
}

#[given("a failing stdlib command helper")]
pub(crate) fn failing_stdlib_command_helper(world: &mut CliWorld) -> Result<()> {
    let root = ensure_workspace(world)?;
    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open stdlib workspace directory")?;
    let helper =
        compile_failure_helper(&handle, &root, "cmd_fail").context("compile failing helper")?;
    world.stdlib_command = Some(format!("\"{}\"", helper.as_str()));
    Ok(())
}

#[given("a large-output stdlib command helper")]
pub(crate) fn large_output_stdlib_command_helper(world: &mut CliWorld) -> Result<()> {
    let root = ensure_workspace(world)?;
    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open stdlib workspace directory")?;
    let helper = compile_large_output_helper(&handle, &root, "cmd_large")
        .context("compile large-output helper")?;
    world.stdlib_command = Some(format!("\"{}\"", helper.as_str()));
    Ok(())
}

#[given(regex = r#"^an HTTP server returning "(.+)"$"#)]
pub(crate) fn http_server_returning(world: &mut CliWorld, body: ServerBody) -> Result<()> {
    let body = body.into_inner();
    world
        .start_http_server(body)
        .context("start stdlib HTTP fixture")?;
    Ok(())
}

#[given(regex = r#"^the stdlib file "(.+)" contains "(.+)"$"#)]
pub(crate) fn write_stdlib_file(
    world: &mut CliWorld,
    path: RelativePath,
    contents: FileContent,
) -> Result<()> {
    let root = ensure_workspace(world)?;
    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open stdlib workspace directory")?;
    let relative_path = TemplatePath::from(path.into_path_buf());
    if let Some(parent) = relative_path
        .as_path()
        .parent()
        .filter(|p| !p.as_str().is_empty())
    {
        handle
            .create_dir_all(parent)
            .context("create stdlib fixture directories")?;
    }
    handle
        .write(relative_path.as_path(), contents.into_bytes())
        .context("write stdlib fixture file")?;
    Ok(())
}

#[given(regex = r#"^the stdlib executable "(.+)" exists$"#)]
pub(crate) fn stdlib_executable_exists(world: &mut CliWorld, path: RelativePath) -> Result<()> {
    let root = ensure_workspace(world)?;
    let relative = path.into_path_buf();
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

#[given(regex = r#"^the stdlib PATH entries are "(.*)"$"#)]
pub(crate) fn stdlib_path_entries(world: &mut CliWorld, entries: PathEntries) -> Result<()> {
    let root = ensure_workspace(world)?;
    let raw_entries = entries.into_inner();
    let trimmed = raw_entries.trim();
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
        OsString::new()
    } else {
        env::join_paths(dirs.iter().map(|dir| dir.as_std_path()))
            .context("join stdlib PATH entries")?
    };
    let previous = set_var("PATH", joined.as_os_str());
    world.env_vars.insert("PATH".into(), previous);
    #[cfg(windows)]
    {
        let previous = set_var("PATHEXT", OsStr::new(".cmd;.exe"));
        world.env_vars.insert("PATHEXT".into(), previous);
    }
    Ok(())
}

#[given("HOME points to the stdlib workspace root")]
pub(crate) fn home_points_to_stdlib_root(world: &mut CliWorld) -> Result<()> {
    let root = ensure_workspace(world)?;
    let os_root = OsStr::new(root.as_str());
    let previous = set_var("HOME", os_root);
    world.env_vars.entry("HOME".into()).or_insert(previous);
    #[cfg(windows)]
    {
        let previous = set_var("USERPROFILE", os_root);
        world
            .env_vars
            .entry("USERPROFILE".into())
            .or_insert(previous);
    }
    world.stdlib_root = Some(root);
    Ok(())
}

pub(crate) fn resolve_template_path(root: &Utf8Path, raw: RelativePath) -> TemplatePath {
    if raw.as_str().starts_with('~') {
        return TemplatePath::from(raw.into_path_buf());
    }
    let candidate = raw.into_path_buf();
    if candidate.is_absolute() {
        TemplatePath::from(candidate)
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
