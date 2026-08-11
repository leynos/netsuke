//! Real-Ninja regressions for command-list shell boundaries.
//!
//! These tests cover syntax which would escape a directly interpolated brace
//! group and therefore require the generated command to evaluate each entry as
//! a complete shell unit.

use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use netsuke::ast::{Recipe, StringOrList};
use netsuke::ir::{Action, BuildEdge, BuildGraph};
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
