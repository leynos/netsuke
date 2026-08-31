//! Real-Ninja coverage for direct target command-list lowering.

use super::{open_temp_workspace, temp_workspace_path};
use anyhow::{Context, Result, ensure};
use minijinja::Environment;
use netsuke::ast::{NetsukeManifest, Recipe};
use netsuke::ir::BuildGraph;
use netsuke::manifest::{self, render_manifest};
use netsuke::ninja_gen::generate;
use std::process::Command;
use tempfile::TempDir;
use test_support::ninja_gen::ninja_integration_setup;

fn rendered_direct_target_manifest() -> Result<NetsukeManifest> {
    let manifest = manifest::from_str(
        r#"
netsuke_version: "1.0.0"
targets:
  - name: result.txt
    sources: input.txt
    vars:
      first: rendered-first
      second: rendered-second
    command:
      - "test -f {{ ins }} && echo '{{ first }}' > {{ outs }}"
      - "echo '{{ second }}' >> {{ outs }}"
"#,
    )?;
    render_manifest(manifest, &Environment::new())
}

fn assert_rendered_direct_target(manifest: &NetsukeManifest) -> Result<()> {
    let target = manifest
        .targets
        .first()
        .context("rendered direct target missing")?;
    let Recipe::Command { command } = &target.recipe else {
        anyhow::bail!("direct target should retain its command recipe");
    };
    ensure!(
        command.to_string_vec()
            == [
                "test -f __NETSUKE_INS_PLACEHOLDER__ && echo 'rendered-first' > __NETSUKE_OUTS_PLACEHOLDER__",
                "echo 'rendered-second' >> __NETSUKE_OUTS_PLACEHOLDER__",
            ],
        "rendered direct-target command entries should preserve declaration order: {command:?}"
    );
    Ok(())
}

fn direct_target_command_list_graph() -> Result<BuildGraph> {
    let rendered = rendered_direct_target_manifest()?;
    assert_rendered_direct_target(&rendered)?;
    let graph = BuildGraph::from_manifest(&rendered)?;
    let action = graph
        .actions
        .values()
        .next()
        .context("direct target action missing")?;
    let Recipe::Command {
        command: lowered_command,
    } = &action.recipe
    else {
        anyhow::bail!("lowered direct target should retain a command recipe");
    };
    ensure!(
        lowered_command.to_string_vec()
            == [
                "test -f input.txt && echo 'rendered-first' > result.txt",
                "echo 'rendered-second' >> result.txt",
            ],
        "IR should interpolate every direct-target entry independently in order: {lowered_command:?}"
    );
    Ok(graph)
}

fn execute_direct_target_command_list(dir: &TempDir, graph: &BuildGraph) -> Result<()> {
    let dir_path = temp_workspace_path(dir)?;
    let handle = open_temp_workspace(dir)?;
    handle
        .write("input.txt", b"input")
        .context("write direct-target input")?;
    handle
        .write("build.ninja", generate(graph)?.as_bytes())
        .context("write generated Ninja file")?;
    let ninja_output = Command::new("ninja")
        .arg("result.txt")
        .current_dir(dir_path.as_std_path())
        .output()
        .context("run real Ninja for direct target command list")?;
    ensure!(
        ninja_output.status.success(),
        "direct target command list should succeed: {ninja_output:?}"
    );
    let result = handle.read_to_string("result.txt")?;
    ensure!(
        result == "rendered-first\nrendered-second\n",
        "target output should prove both entries executed in declaration order, got {result:?}"
    );
    Ok(())
}

#[test]
fn direct_target_command_list_renders_lowers_and_executes_in_order() -> Result<()> {
    let Some(dir) = ninja_integration_setup() else {
        return Ok(());
    };
    let graph = direct_target_command_list_graph()?;
    execute_direct_target_command_list(&dir, &graph)
}
