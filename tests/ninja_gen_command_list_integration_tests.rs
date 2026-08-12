//! Real-Ninja regressions for command-list shell boundaries.
//!
//! These tests cover syntax which would escape a directly interpolated brace
//! group and therefore require the generated command to evaluate each entry as
//! a complete shell unit.

use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::Environment;
use netsuke::ast::{NetsukeManifest, Recipe, StringOrList};
use netsuke::ir::{Action, BuildEdge, BuildGraph};
use netsuke::manifest::{self, render_manifest};
use netsuke::ninja_gen::generate;
use std::process::Command;
use tempfile::TempDir;
use test_support::ninja_gen::ninja_integration_setup;

fn run_command_list(
    dir: &TempDir,
    entries: Vec<String>,
    expected_file: &str,
    expected_content: &str,
) -> Result<()> {
    let dir_path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("temp dir path {path:?} is not UTF-8"))?;
    let action = Action {
        recipe: Recipe::Command {
            command: StringOrList::List(entries),
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let target = Utf8PathBuf::from("out");
    let edge = BuildEdge {
        action_id: "chain".into(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        explicit_outputs: vec![target.clone()],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("chain".into(), action);
    graph.targets.insert(target.clone(), edge);
    graph.default_targets.push(target);

    let ninja = generate(&graph)?;
    let handle = Dir::open_ambient_dir(&dir_path, ambient_authority())
        .with_context(|| format!("open ambient dir for temp workspace at {dir_path}"))?;
    handle
        .write("build.ninja", ninja.as_bytes())
        .context("write ninja build file")?;
    let ninja_output = Command::new("ninja")
        .arg("out")
        .current_dir(dir_path.as_std_path())
        .output()
        .context("invoke ninja")?;
    ensure!(
        ninja_output.status.success(),
        "command list should run successfully: {ninja_output:?}"
    );
    let content = handle
        .read_to_string(expected_file)
        .with_context(|| format!("read {expected_file} written by the second entry"))?;
    ensure!(
        content.trim() == expected_content,
        "expected {expected_file} to contain '{expected_content}', got '{content}'"
    );
    Ok(())
}

#[test]
fn command_list_entry_with_inline_comment_preserves_the_next_boundary() -> Result<()> {
    let Some(dir) = ninja_integration_setup() else {
        return Ok(());
    };
    run_command_list(
        &dir,
        vec![
            "echo first # a comment that formerly consumed the closing brace".into(),
            "echo second > after-comment.txt".into(),
        ],
        "after-comment.txt",
        "second",
    )
}

#[test]
fn command_list_entry_ending_in_background_operator_preserves_the_next_boundary() -> Result<()> {
    let Some(dir) = ninja_integration_setup() else {
        return Ok(());
    };
    run_command_list(
        &dir,
        vec!["true &".into(), "echo second > after-background.txt".into()],
        "after-background.txt",
        "second",
    )
}

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
      - "test -f $in && echo '{{ first }}' > $out"
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
                "test -f $in && echo 'rendered-first' > $out",
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
    let dir_path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("temp dir path {path:?} is not UTF-8"))?;

    let handle = Dir::open_ambient_dir(&dir_path, ambient_authority())
        .with_context(|| format!("open ambient dir for temp workspace at {dir_path}"))?;
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
