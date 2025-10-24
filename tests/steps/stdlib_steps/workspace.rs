use crate::CliWorld;
use anyhow::{Context, Result, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use cucumber::given;
use std::ffi::OsStr;
use test_support::{
    command_helper::{compile_failure_helper, compile_uppercase_helper},
    env::set_var,
};

use super::types::{FileContent, RelativePath, TemplatePath};

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

#[given(regex = r#"^an HTTP server returning "(.+)"$"#)]
pub(crate) fn http_server_returning(world: &mut CliWorld, body: String) -> Result<()> {
    world
        .start_http_server(body)
        .context("start stdlib HTTP fixture")?;
    Ok(())
}

#[given(regex = r#"^the stdlib file "(.+)" contains "(.+)"$"#)]
pub(crate) fn write_stdlib_file(
    world: &mut CliWorld,
    path: String,
    contents: String,
) -> Result<()> {
    let root = ensure_workspace(world)?;
    let handle = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open stdlib workspace directory")?;
    let relative_path = TemplatePath::from(path);
    let file_content = FileContent::from(contents);
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
        .write(relative_path.as_path(), file_content.as_bytes())
        .context("write stdlib fixture file")?;
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

pub(crate) fn resolve_template_path(root: &Utf8Path, raw: &RelativePath) -> TemplatePath {
    if raw.as_str().starts_with('~') {
        return TemplatePath::from(raw.as_str().to_owned());
    }
    let candidate = raw.to_path_buf();
    if candidate.is_absolute() {
        TemplatePath::from(candidate)
    } else {
        TemplatePath::from(root.join(candidate))
    }
}
