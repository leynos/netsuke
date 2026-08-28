//! Step definitions for the `netsuke help targets` full-process scenarios.

use crate::bdd::fixtures::TestWorld;
use crate::bdd::steps::manifest_command::manifest_command_helpers::run_netsuke_and_store;
use anyhow::{Context, Result};
use rstest::fixture;
use rstest_bdd_macros::{given, when};
use std::{fs, path::PathBuf};

/// Install an isolated workspace and reset command observations for one scenario.
#[fixture]
fn help_targets_workspace() -> impl Fn(&TestWorld) -> Result<PathBuf> {
    |world| {
        let temp = tempfile::tempdir().context("create temp dir for help-target workspace")?;
        let workspace_path = temp.path().to_path_buf();
        *world.temp_dir.borrow_mut() = Some(temp);
        world.run_status.clear();
        world.run_error.clear();
        world.command_stdout.clear();
        world.command_stderr.clear();
        Ok(workspace_path)
    }
}

#[given("a Netsuke workspace with described actions and targets")]
fn described_actions_and_targets_workspace(world: &TestWorld) -> Result<()> {
    let manifest = help_targets_workspace()(world)?.join("Netsukefile");
    fs::write(
        &manifest,
        r#"netsuke_version: "1.0.0"
actions:
  - name: lint
    description: Run rustdoc, Clippy, and Whitaker
    command: cargo clippy
  - name: test
    description: Run unit, behavioural, UI, and documentation tests
    command: cargo test
targets:
  - name: target/release/catnap
    description: Build the optimized release binary
    command: cargo build --release
defaults:
  - lint
  - test
"#,
    )
    .with_context(|| format!("write manifest to {}", manifest.display()))?;
    Ok(())
}

#[given("a Netsuke workspace with a conditional action")]
fn conditional_action_workspace(world: &TestWorld) -> Result<()> {
    let manifest = help_targets_workspace()(world)?.join("Netsukefile");
    fs::write(
        &manifest,
        r#"netsuke_version: "1.0.0"
actions:
  - name: preferred
    description: Run tests with cargo-nextest
    command: touch preferred-ran
    when: command_available("cargo-nextest")
  - name: fallback
    description: Run tests with Cargo
    command: touch fallback-ran
    when: not command_available("cargo-nextest")
targets: []
"#,
    )
    .with_context(|| format!("write manifest to {}", manifest.display()))?;
    Ok(())
}

#[when("the netsuke help targets subcommand is run")]
fn run_help_targets_subcommand(world: &TestWorld) -> Result<()> {
    run_netsuke_and_store(world, &["help", "targets"])
}
