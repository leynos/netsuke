//! Real-resolver coverage for complementary action selection.

use std::ffi::OsString;

use anyhow::{Context, Result, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
#[cfg(unix)]
use cap_std::fs::PermissionsExt;
use cap_std::{ambient_authority, fs_utf8::Dir};
use googletest::prelude::*;
use minijinja::Environment;
use rstest::rstest;

use super::{actions, expand_foreach};
use crate::{
    manifest::ManifestValue,
    stdlib::{self, StdlibConfig},
};

const ABSENT_COMMAND: &str = "netsuke-guaranteed-absent-command-3-14-5";

struct ExpectedAction {
    name: &'static str,
    command: &'static str,
    has_iteration_vars: bool,
}

impl ExpectedAction {
    const fn nextest() -> Self {
        Self {
            name: "run-tests-nextest",
            command: "cargo nextest run",
            has_iteration_vars: true,
        }
    }

    const fn legacy() -> Self {
        Self {
            name: "run-tests-legacy",
            command: "cargo test",
            has_iteration_vars: false,
        }
    }
}

struct ResolverWorld {
    _temp: tempfile::TempDir,
    root: Utf8PathBuf,
}

impl ResolverWorld {
    fn new(has_nextest: bool) -> Result<Self> {
        let temp = tempfile::tempdir().context("create resolver workspace")?;
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .map_err(|path| anyhow!("temporary path should be UTF-8: {}", path.display()))?;
        if has_nextest {
            write_tool(&root, "cargo-nextest")?;
        }
        Ok(Self { _temp: temp, root })
    }

    fn environment(&self, has_nextest: bool) -> Result<Environment<'static>> {
        let workspace = Dir::open_ambient_dir(&self.root, ambient_authority())
            .with_context(|| format!("open resolver workspace {}", self.root))?;
        let path_override = if has_nextest {
            OsString::from(self.root.as_str())
        } else {
            OsString::new()
        };
        let config = StdlibConfig::new(workspace)?
            .with_workspace_root_path(self.root.clone())?
            .with_path_override(path_override)
            .with_pathext_override(".CMD");
        let mut env = Environment::new();
        stdlib::register_with_config(&mut env, config)?;
        Ok(env)
    }
}

#[googletest::test]
#[rstest]
#[case::nextest_present(true, "cargo-nextest", ExpectedAction::nextest())]
#[case::nextest_absent(false, ABSENT_COMMAND, ExpectedAction::legacy())]
fn real_command_available_selects_exactly_one_action(
    #[case] has_nextest: bool,
    #[case] command_name: &str,
    #[case] expected: ExpectedAction,
) -> googletest::Result<()> {
    let world = ResolverWorld::new(has_nextest).or_fail()?;
    let env = world.environment(has_nextest).or_fail()?;
    let yaml = format!(
        "actions:
  - name: run-tests-nextest
    command: cargo nextest run
    foreach: [nextest]
    when: command_available({command_name:?}, cwd_mode='never')
  - name: run-tests-legacy
    command: cargo test
    when: not command_available({command_name:?}, cwd_mode='never')
targets: []
"
    );
    let mut doc: ManifestValue = serde_saphyr::from_str(&yaml).or_fail()?;

    expand_foreach(&mut doc, &env).or_fail()?;

    let selected_actions = actions(&doc).or_fail()?;
    verify_that!(selected_actions, len(eq(1)))?;
    let selected = selected_actions.first().or_fail()?;
    let selected_map = selected.as_object().or_fail()?;
    verify_that!(string_field(selected, "name").or_fail()?, eq(expected.name))?;
    verify_that!(
        string_field(selected, "command").or_fail()?,
        eq(expected.command)
    )?;
    verify_that!(selected_map.contains_key("when"), eq(false))?;
    verify_that!(selected_map.contains_key("foreach"), eq(false))?;
    verify_that!(
        selected_map.contains_key("vars"),
        eq(expected.has_iteration_vars)
    )?;
    Ok(())
}

fn string_field<'a>(value: &'a ManifestValue, field: &str) -> Result<&'a str> {
    value
        .as_object()
        .context("selected action map")?
        .get(field)
        .and_then(ManifestValue::as_str)
        .with_context(|| format!("selected action {field}"))
}

fn write_tool(root: &Utf8Path, name: &str) -> Result<()> {
    let dir = Dir::open_ambient_dir(root, ambient_authority())
        .with_context(|| format!("open fixture directory {root}"))?;
    let path = Utf8PathBuf::from(tool_filename(name));
    dir.write(&path, script_contents())
        .with_context(|| format!("write fixture tool {path}"))?;
    #[cfg(unix)]
    mark_executable(&dir, &path)?;
    Ok(())
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
fn mark_executable(dir: &Dir, path: &Utf8Path) -> Result<()> {
    let mut permissions = dir
        .metadata(path)
        .with_context(|| format!("stat {path}"))?
        .permissions();
    permissions.set_mode(0o755);
    dir.set_permissions(path, permissions)
        .with_context(|| format!("chmod {path}"))
}
