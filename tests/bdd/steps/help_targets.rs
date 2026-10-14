//! Step definitions for the `netsuke help targets` full-process scenarios.

use crate::bdd::fixtures::TestWorld;
use crate::bdd::steps::manifest_command::manifest_command_helpers::run_netsuke_and_store;
use anyhow::{Context, Result, ensure};
use rstest::fixture;
use rstest_bdd_macros::{given, then, when};
use serde_json::Value;
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

/// Create a workspace containing actions and targets with descriptions.
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

/// Create a workspace containing a conditional action for help-target scenarios.
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

/// Create a workspace with raw recipe helpers and all three dependency edge kinds.
#[given("a Netsuke workspace with graph export edges")]
fn graph_export_workspace(world: &TestWorld) -> Result<()> {
    let manifest = help_targets_workspace()(world)?.join("Netsukefile");
    fs::write(
        &manifest,
        r#"netsuke_version: "1.0.0"
rules:
  - name: compile
    command: 'echo {{ fetch("https://example.invalid/source", cache=true) }}'
actions:
  - name: inspect
    command: 'echo {{ command_available("compiler") }} {{ env("NETSUKE_GRAPH_EXPORT_SECRET") }} {{ "ignored" | shell("touch shell-ran") }} {{ ins }} {{ outs }}'
targets:
  - name: input
    command: touch input-ran
  - name: ready
    command: touch ready-ran
  - name: output
    rule: compile
    sources: [source.c]
    deps: [input]
    order_only_deps: [ready]
"#,
    )
    .with_context(|| format!("write graph-export manifest to {}", manifest.display()))?;
    Ok(())
}

#[when("the netsuke help targets subcommand is run")]
fn run_help_targets_subcommand(world: &TestWorld) -> Result<()> {
    run_netsuke_and_store(world, &["help", "targets"])
}

/// Run the opt-in graph-bearing help query as a child process.
#[when("the netsuke help targets graph export is run")]
fn run_help_targets_graph_export(world: &TestWorld) -> Result<()> {
    run_netsuke_and_store(world, &["--json", "help", "targets", "--edges"])
}

/// Verify the graph result keeps recipe helpers raw and emits every edge class.
#[then("the JSON help-target graph should contain raw recipes and edge kinds")]
fn graph_export_contains_raw_recipes_and_edges(world: &TestWorld) -> Result<()> {
    let stdout = world
        .command_stdout
        .get()
        .context("graph export stdout should be captured")?;
    let document: Value = serde_json::from_str(&stdout).context("parse graph export JSON")?;
    let action = document
        .pointer("/result/graph/nodes/0/recipe/commands/0")
        .and_then(Value::as_str)
        .context("graph export should contain the action command")?;
    for helper in [
        "command_available",
        "env(",
        "shell(",
        "{{ ins }}",
        "{{ outs }}",
    ] {
        ensure!(
            action.contains(helper),
            "raw action recipe should preserve {helper:?}: {action}"
        );
    }
    let rule = document
        .pointer("/result/graph/rules/0/recipe/commands/0")
        .and_then(Value::as_str)
        .context("graph export should contain the rule command")?;
    ensure!(
        rule.contains("{{ fetch("),
        "raw rule recipe should preserve fetch: {rule}"
    );
    for path in [
        "/result/graph/nodes/3/edges/sources/0",
        "/result/graph/nodes/3/edges/deps/0",
        "/result/graph/nodes/3/edges/order_only_deps/0",
    ] {
        ensure!(
            document.pointer(path).and_then(Value::as_str).is_some(),
            "graph export should include edge kind at {path}"
        );
    }
    Ok(())
}
