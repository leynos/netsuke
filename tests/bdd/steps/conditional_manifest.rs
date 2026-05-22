//! Step definitions for conditional manifest planning scenarios.
//!
//! These BDD steps build temporary manifests that exercise conditional
//! `foreach` and `when` handling at manifest time. The scenarios verify both
//! branch selection for actions and targets and the `command_available(...)`
//! predicate used by complementary branches.
//!
//! Each step stores its temporary workspace, manifest, and command-path
//! changes through [`TestWorld`]. That keeps the scenario state isolated while
//! still letting later assertions inspect the command outputs and environment
//! mutations created by the manifest under test.

use crate::bdd::fixtures::TestWorld;
use anyhow::{Context, Result};
use rstest_bdd_macros::given;
use std::fs;
use std::path::Path;

const CONDITIONAL_MANIFEST: &str = r#"netsuke_version: "1.0.0"
actions:
  - foreach:
      - kept
      - skipped
    when: item != 'skipped'
    name: "action-{{ item }}"
    command: "echo action-{{ item }}"
targets:
  - foreach:
      - kept
      - skipped
    when: item != 'skipped'
    name: "target-{{ item }}"
    command: "echo target-{{ item }}"
"#;

const COMMAND_AVAILABLE_MANIFEST: &str = r#"netsuke_version: "1.0.0"
actions:
  - name: "preferred-action"
    command: "echo preferred"
    when: command_available("preferred-tool")
  - name: "fallback-action"
    command: "echo fallback"
    when: not command_available("preferred-tool")
targets:
  - name: "done"
    command: "true"
"#;

fn reset_command_state(world: &TestWorld) {
    world.run_status.clear();
    world.run_error.clear();
    world.command_stdout.clear();
    world.command_stderr.clear();
}

fn write_executable(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .context("executable path should have parent")?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    fs::write(path, executable_script()).with_context(|| format!("write {}", path.display()))?;
    mark_executable(path)
}

fn fixture_command_path(bin: &Path, name: &str) -> std::path::PathBuf {
    if cfg!(windows) {
        bin.join(format!("{name}.cmd"))
    } else {
        bin.join(name)
    }
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

#[cfg(unix)]
fn mark_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)
        .with_context(|| format!("stat {}", path.display()))?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).with_context(|| format!("chmod {}", path.display()))
}

#[cfg(not(unix))]
fn mark_executable(_path: &Path) -> Result<()> {
    Ok(())
}

fn prepend_path_for_child(world: &TestWorld, dir: &Path) -> Result<()> {
    let mut entries = vec![dir.to_path_buf()];
    if let Some(host_path) = std::env::var_os("PATH") {
        entries.extend(std::env::split_paths(&host_path));
    }
    let joined = std::env::join_paths(entries).context("join PATH entries")?;
    world.track_env_var("PATH".to_owned(), std::env::var_os("PATH"), Some(joined));
    Ok(())
}

/// Create a workspace whose manifest contains conditional actions and targets.
#[given("a Netsuke workspace with conditional actions and targets")]
fn conditional_actions_and_targets_workspace(world: &TestWorld) -> Result<()> {
    let temp = tempfile::tempdir().context("create temp dir for conditional manifest")?;
    let netsukefile = temp.path().join("Netsukefile");
    fs::write(&netsukefile, CONDITIONAL_MANIFEST)
        .with_context(|| format!("write manifest to {}", netsukefile.display()))?;
    *world.temp_dir.borrow_mut() = Some(temp);
    reset_command_state(world);
    Ok(())
}

/// Create a workspace with complementary command-availability action branches.
#[given("a Netsuke workspace with a preferred command available")]
fn command_available_actions_workspace(world: &TestWorld) -> Result<()> {
    let temp = tempfile::tempdir().context("create temp dir for command-available manifest")?;
    let netsukefile = temp.path().join("Netsukefile");
    fs::write(&netsukefile, COMMAND_AVAILABLE_MANIFEST)
        .with_context(|| format!("write manifest to {}", netsukefile.display()))?;
    let bin = temp.path().join("bin");
    let tool = fixture_command_path(&bin, "preferred-tool");
    write_executable(&tool)?;
    prepend_path_for_child(world, &bin)?;
    *world.temp_dir.borrow_mut() = Some(temp);
    reset_command_state(world);
    Ok(())
}
