//! Boundary coverage for pure command-availability selection.

use std::ffi::OsString;

use anyhow::{Context, Result, anyhow};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use googletest::prelude::*;
use minijinja::Environment;
use rstest::rstest;

use super::{actions, expand_foreach};
use crate::{
    manifest::ManifestValue,
    stdlib::{self, StdlibConfig, StdlibState},
};

const ABSENT_COMMAND: &str = "netsuke-guaranteed-absent-command-no-shell-3-14-5";

struct ExpansionWorld {
    _temp: tempfile::TempDir,
    environment: Environment<'static>,
    state: StdlibState,
}

impl ExpansionWorld {
    fn new() -> Result<Self> {
        let temp = tempfile::tempdir().context("create expansion workspace")?;
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .map_err(|path| anyhow!("temporary path should be UTF-8: {path:?}"))?;
        let workspace = Dir::open_ambient_dir(&root, ambient_authority())
            .with_context(|| format!("open expansion workspace {root}"))?;
        let config = StdlibConfig::new(workspace)?
            .with_workspace_root_path(root)?
            .with_path_override(OsString::new());
        let mut environment = Environment::new();
        let state = stdlib::register_with_config(&mut environment, config)?;
        Ok(Self {
            _temp: temp,
            environment,
            state,
        })
    }
}

#[googletest::test]
#[rstest]
fn command_available_fallback_does_not_invoke_impure_helpers() -> googletest::Result<()> {
    let world = ExpansionWorld::new().or_fail()?;
    let yaml = format!(
        "actions:
  - name: unavailable-tool
    command: unavailable-tool
    when: command_available({ABSENT_COMMAND:?}, cwd_mode='never')
  - name: fallback
    command: cargo test
    when: not command_available({ABSENT_COMMAND:?}, cwd_mode='never')
targets: []
"
    );
    let mut doc: ManifestValue = serde_saphyr::from_str(&yaml).or_fail()?;

    expand_foreach(&mut doc, &world.environment).or_fail()?;

    let selected_actions = actions(&doc).or_fail()?;
    verify_that!(selected_actions, len(eq(1)))?;
    let selected = selected_actions.first().or_fail()?;
    let selected_map = selected.as_object().or_fail()?;
    verify_that!(string_field(selected, "name").or_fail()?, eq("fallback"))?;
    verify_that!(selected_map.contains_key("when"), eq(false))?;
    verify_that!(world.state.is_impure(), eq(false))?;
    Ok(())
}

#[googletest::test]
#[rstest]
fn shell_in_when_marks_selection_impure() -> googletest::Result<()> {
    let world = ExpansionWorld::new().or_fail()?;
    let command = shell_control_command();
    let yaml = format!(
        "actions:
  - name: shell-control
    command: echo control
    when: >-
      ('' | shell({command:?})) == ''
targets: []
"
    );
    let mut doc: ManifestValue = serde_saphyr::from_str(&yaml).or_fail()?;

    expand_foreach(&mut doc, &world.environment).or_fail()?;

    verify_that!(actions(&doc).or_fail()?, len(eq(1)))?;
    verify_that!(world.state.is_impure(), eq(true))?;
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

const fn shell_control_command() -> &'static str {
    #[cfg(windows)]
    {
        "exit /b 0"
    }
    #[cfg(not(windows))]
    {
        ":"
    }
}
