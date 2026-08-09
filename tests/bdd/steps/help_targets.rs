//! Step definitions for the `netsuke help targets` full-process scenarios.

use crate::bdd::fixtures::TestWorld;
use crate::bdd::steps::manifest_command::manifest_command_helpers::run_netsuke_and_store;
use anyhow::{Context, Result};
use rstest_bdd_macros::{given, when};
use std::fs;

#[given("a Netsuke workspace with described actions and targets")]
fn described_actions_and_targets_workspace(world: &TestWorld) -> Result<()> {
    let temp = tempfile::tempdir().context("create temp dir for described workspace")?;
    let manifest = temp.path().join("Netsukefile");
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
    *world.temp_dir.borrow_mut() = Some(temp);
    world.run_status.clear();
    world.run_error.clear();
    world.command_stdout.clear();
    world.command_stderr.clear();
    Ok(())
}

#[when("the netsuke help targets subcommand is run")]
fn run_help_targets_subcommand(world: &TestWorld) -> Result<()> {
    run_netsuke_and_store(world, &["help", "targets"])
}
