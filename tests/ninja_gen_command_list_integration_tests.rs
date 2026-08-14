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
use netsuke::ninja_gen::{NinjaGenError, generate};
use std::process::Command;
use tempfile::TempDir;
use test_support::ninja_gen::ninja_integration_setup;

fn run_command_list(
    dir: &TempDir,
    entries: Vec<String>,
    expected_file: &str,
    expected_content: &str,
) -> Result<()> {
    let dir_path = temp_workspace_path(dir)?;
    let action = command_list_action(entries);
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
    let handle = open_temp_workspace(dir)?;
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
        vec![
            "(sleep 1 && echo waited > waited-background-job.txt) &".into(),
            "echo second > after-background.txt".into(),
        ],
        "after-background.txt",
        "second",
    )?;
    let workspace = open_temp_workspace(&dir)?;
    ensure!(
        workspace.exists("waited-background-job.txt"),
        "Ninja must wait for a successful background job before running the next entry"
    );
    Ok(())
}

const fn command_list_action(entries: Vec<String>) -> Action {
    Action {
        recipe: Recipe::Command {
            command: StringOrList::List(entries),
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    }
}

fn command_list_command_line(entries: Vec<String>) -> Result<String> {
    let action = command_list_action(entries);
    let mut graph = BuildGraph::default();
    graph.actions.insert("chain".into(), action);
    let ninja = generate(&graph)?;
    ninja
        .lines()
        .find_map(|line| line.strip_prefix("  command = "))
        .map(str::to_owned)
        .context("generated command-list action missing")
}

fn assert_multiple_background_jobs_are_rejected(
    entries: Vec<String>,
    expectation: &str,
) -> Result<()> {
    let Err(error) = command_list_command_line(entries) else {
        anyhow::bail!("{expectation}");
    };
    ensure!(
        matches!(
            error.downcast_ref::<NinjaGenError>(),
            Some(NinjaGenError::MultipleBackgroundJobs {
                action_index: 1,
                entry_index: 1,
            })
        ),
        "multiple background jobs should return a stable typed error: {error:?}"
    );
    Ok(())
}

fn open_temp_workspace(dir: &TempDir) -> Result<Dir> {
    let dir_path = temp_workspace_path(dir)?;
    Dir::open_ambient_dir(&dir_path, ambient_authority()).context("open command-list workspace")
}

fn temp_workspace_path(dir: &TempDir) -> Result<Utf8PathBuf> {
    let dir_path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("temp dir path {path:?} is not UTF-8"))?;
    Ok(dir_path)
}

fn run_generated_command_with_ninja(dir: &TempDir, command: &str) -> Result<std::process::Output> {
    let workspace = open_temp_workspace(dir)?;
    workspace.write(
        "build.ninja",
        format!("rule chain\n  command = {command}\nbuild out: chain\n").as_bytes(),
    )?;
    Command::new("ninja")
        .arg("out")
        .current_dir(dir.path())
        .output()
        .context("run generated command-list with Ninja")
}

#[test]
fn command_list_exit_entry_preserves_status_and_emits_attribution() -> Result<()> {
    let Some(dir) = ninja_integration_setup() else {
        return Ok(());
    };
    let command = command_list_command_line(vec![
        "exit 23".into(),
        "echo unexpected > continued-after-exit.txt".into(),
    ])?;
    let shell_command = command.replace("$$", "$");
    let output = Command::new("sh")
        .args(["-c", &shell_command])
        .current_dir(dir.path())
        .output()
        .context("run generated command-list shell")?;
    ensure!(
        output.status.code() == Some(23),
        "exit command should retain status 23, got {:?}",
        output.status
    );
    let stderr = String::from_utf8(output.stderr).context("shell stderr should be UTF-8")?;
    ensure!(
        stderr.contains("netsuke command-list failure: action ") && stderr.contains(", entry 1"),
        "exit command should emit the first-entry marker: {stderr}"
    );
    let workspace = open_temp_workspace(&dir)?;
    ensure!(
        !workspace.exists("continued-after-exit.txt"),
        "an exit failure must not run a later entry"
    );
    let ninja_output = run_generated_command_with_ninja(&dir, &command)?;
    ensure!(
        !ninja_output.status.success(),
        "Ninja should report the generated command failure, got {:?}",
        ninja_output.status
    );
    let ninja_diagnostics = format!(
        "{}{}",
        String::from_utf8(ninja_output.stdout).context("Ninja stdout should be UTF-8")?,
        String::from_utf8(ninja_output.stderr).context("Ninja stderr should be UTF-8")?,
    );
    ensure!(
        ninja_diagnostics.contains(", entry 1"),
        "Ninja should retain the first-entry marker: {ninja_diagnostics}"
    );
    Ok(())
}

#[test]
fn command_list_exec_entries_preserve_attribution_and_success() -> Result<()> {
    let Some(dir) = ninja_integration_setup() else {
        return Ok(());
    };
    let successful_command = command_list_command_line(vec![
        "exec true".into(),
        "echo unexpected > continued-after-successful-exec.txt".into(),
    ])?;
    let successful_output = run_generated_command_with_ninja(&dir, &successful_command)?;
    ensure!(
        successful_output.status.success(),
        "a successful process-replacing entry must succeed"
    );
    let successful_diagnostics = format!(
        "{}{}",
        String::from_utf8(successful_output.stdout).context("Ninja stdout should be UTF-8")?,
        String::from_utf8(successful_output.stderr).context("Ninja stderr should be UTF-8")?,
    );
    ensure!(
        !successful_diagnostics
            .lines()
            .any(|line| line.starts_with("netsuke command-list failure: action ")),
        "successful exec must not emit failure attribution: {successful_diagnostics}"
    );
    let workspace = open_temp_workspace(&dir)?;
    ensure!(
        !workspace.exists("continued-after-successful-exec.txt"),
        "a successful exec must retain process-replacement semantics"
    );
    let command = command_list_command_line(vec![
        "FOO=1 exec false".into(),
        "echo unexpected > continued-after-exec.txt".into(),
    ])?;
    let output = run_generated_command_with_ninja(&dir, &command)?;
    ensure!(
        !output.status.success(),
        "a process-replacing entry must fail the Ninja build"
    );
    let stdout = String::from_utf8(output.stdout).context("Ninja stdout should be UTF-8")?;
    let stderr = String::from_utf8(output.stderr).context("Ninja stderr should be UTF-8")?;
    let diagnostics = format!("{stdout}{stderr}");
    ensure!(
        diagnostics.contains("netsuke command-list failure: action ")
            && diagnostics.contains(", entry 1"),
        "assignment-prefixed exec failure should emit the first-entry marker: {diagnostics}"
    );
    ensure!(
        !workspace.exists("continued-after-exec.txt"),
        "an exec failure must not run a later entry"
    );
    Ok(())
}

#[test]
fn command_list_background_failure_waits_before_the_next_entry() -> Result<()> {
    let Some(dir) = ninja_integration_setup() else {
        return Ok(());
    };
    let command = command_list_command_line(vec![
        "sh -c 'sleep 0.1; exit 1' &".into(),
        "echo unexpected > continued-after-background.txt".into(),
    ])?;
    let shell_command = command.replace("$$", "$");
    let output = Command::new("sh")
        .args(["-c", &shell_command])
        .current_dir(dir.path())
        .output()
        .context("run generated command-list shell")?;
    ensure!(
        !output.status.success(),
        "failing background work must fail its command-list entry"
    );
    let stderr = String::from_utf8(output.stderr).context("shell stderr should be UTF-8")?;
    ensure!(
        stderr.contains(", entry 1"),
        "background failure should identify the first entry: {stderr}"
    );
    let workspace = open_temp_workspace(&dir)?;
    ensure!(
        !workspace.exists("continued-after-background.txt"),
        "a background failure must stop later entries"
    );
    let ninja_output = run_generated_command_with_ninja(&dir, &command)?;
    ensure!(
        !ninja_output.status.success(),
        "Ninja must fail when a backgrounded entry fails: {ninja_output:?}"
    );
    Ok(())
}

#[test]
fn command_list_rejects_multiple_background_jobs() -> Result<()> {
    assert_multiple_background_jobs_are_rejected(
        vec![
            "true & sh -c 'sleep 0.1; exit 1' &".into(),
            "echo unexpected > continued-after-multiple-background-jobs.txt".into(),
        ],
        "multiple background jobs should be rejected before Ninja runs",
    )
}

#[test]
fn command_list_rejects_nested_eval_background_jobs_before_later_entries() -> Result<()> {
    assert_multiple_background_jobs_are_rejected(
        vec![
            "eval 'false & true &'".into(),
            "echo unexpected > continued-after-nested-eval.txt".into(),
        ],
        "nested eval background jobs should be rejected before Ninja runs",
    )
}

#[path = "support/ninja_gen_direct_target_command_list.rs"]
mod direct_target_tests;
