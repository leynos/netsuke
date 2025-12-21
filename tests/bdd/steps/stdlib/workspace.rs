//! Helpers for preparing stdlib workspaces during BDD scenarios, wiring
//! up temporary directories, fixtures, and environment overrides for tests.

use crate::bdd::fixtures::{RefCellOptionExt, strip_quotes, with_world};
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

pub(crate) fn ensure_workspace() -> Result<Utf8PathBuf> {
    with_world(|world| {
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
    })
}

#[given("a stdlib workspace")]
pub(crate) fn stdlib_workspace() -> Result<()> {
    let root = ensure_workspace()?;
    with_world(|world| {
        world.stdlib_root.set(root);
    });
    Ok(())
}

#[given("an uppercase stdlib command helper")]
pub(crate) fn uppercase_stdlib_command_helper() -> Result<()> {
    let root = ensure_workspace()?;
    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open stdlib workspace directory")?;
    let helper = compile_uppercase_helper(&handle, &root, "cmd_upper")
        .context("compile uppercase helper")?;
    with_world(|world| {
        world.stdlib_command.set(format!("\"{}\"", helper.as_str()));
    });
    Ok(())
}

#[given("a failing stdlib command helper")]
pub(crate) fn failing_stdlib_command_helper() -> Result<()> {
    let root = ensure_workspace()?;
    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open stdlib workspace directory")?;
    let helper =
        compile_failure_helper(&handle, &root, "cmd_fail").context("compile failing helper")?;
    with_world(|world| {
        world.stdlib_command.set(format!("\"{}\"", helper.as_str()));
    });
    Ok(())
}

#[given("a large-output stdlib command helper")]
pub(crate) fn large_output_stdlib_command_helper() -> Result<()> {
    let root = ensure_workspace()?;
    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open stdlib workspace directory")?;
    let helper = compile_large_output_helper(&handle, &root, "cmd_large")
        .context("compile large-output helper")?;
    with_world(|world| {
        world.stdlib_command.set(format!("\"{}\"", helper.as_str()));
    });
    Ok(())
}

#[given("an HTTP server returning {body}")]
pub(crate) fn http_server_returning(body: String) -> Result<()> {
    let body = strip_quotes(&body).to_string();
    with_world(|world| {
        world.shutdown_http_server();
        let (url, server) = test_support::http::spawn_http_server(body)
            .context("spawn HTTP server for stdlib steps")?;
        world.stdlib_url.set(url);
        world.http_server.set_value(server);
        Ok(())
    })
}

#[given("the stdlib file {path} contains {contents}")]
pub(crate) fn write_stdlib_file(path: String, contents: String) -> Result<()> {
    let path = strip_quotes(&path);
    let contents = strip_quotes(&contents);
    let root = ensure_workspace()?;
    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open stdlib workspace directory")?;
    let relative_path = TemplatePath::from(path);
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
        .write(relative_path.as_path(), contents.as_bytes())
        .context("write stdlib fixture file")?;
    Ok(())
}

#[given("the stdlib executable {path} exists")]
pub(crate) fn stdlib_executable_exists(path: String) -> Result<()> {
    let path = strip_quotes(&path);
    let root = ensure_workspace()?;
    let relative = Utf8PathBuf::from(path);
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

#[given("the stdlib PATH entries are {entries}")]
pub(crate) fn stdlib_path_entries(entries: String) -> Result<()> {
    let raw_entries = strip_quotes(&entries);
    let root = ensure_workspace()?;
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
        std::ffi::OsString::new()
    } else {
        env::join_paths(dirs.iter().map(|dir| dir.as_std_path()))
            .context("join stdlib PATH entries")?
    };
    let previous = set_var("PATH", joined.as_os_str());
    with_world(|world| {
        world.track_env_var("PATH".into(), previous);
    });
    #[cfg(windows)]
    {
        let previous = set_var("PATHEXT", OsStr::new(".cmd;.exe"));
        with_world(|world| {
            world.track_env_var("PATHEXT".into(), previous);
        });
    }
    Ok(())
}

#[given("HOME points to the stdlib workspace root")]
pub(crate) fn home_points_to_stdlib_root() -> Result<()> {
    let root = ensure_workspace()?;
    let os_root = OsStr::new(root.as_str());
    let previous = set_var("HOME", os_root);
    with_world(|world| {
        world.track_env_var("HOME".into(), previous);
    });
    #[cfg(windows)]
    {
        let previous = set_var("USERPROFILE", os_root);
        with_world(|world| {
            world.track_env_var("USERPROFILE".into(), previous);
        });
    }
    with_world(|world| {
        world.stdlib_root.set(root);
    });
    Ok(())
}

pub(crate) fn resolve_template_path(root: &Utf8Path, raw: &str) -> TemplatePath {
    if raw.starts_with('~') {
        return TemplatePath::from(raw);
    }
    let candidate = Utf8PathBuf::from(raw);
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
