//! Real-Ninja command-list process-boundary regressions.
//!
//! These cases exercise failure propagation and shell process reuse through a
//! generated Ninja file, separately from the broader generator integration
//! scenarios.

#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use netsuke::ast::{CommandIsolation, Recipe, StringOrList};
use netsuke::ir::{Action, BuildEdge, BuildGraph, DependencyOrder};
use netsuke::ninja_gen::generate;
use rstest::{fixture, rstest};
use std::process::Command;
use tempfile::TempDir;
use test_support::ninja_gen;

#[fixture]
fn ninja_integration_setup() -> Option<TempDir> {
    ninja_gen::ninja_integration_setup()
}

struct CommandListRun {
    _workspace: TempDir,
    dir: Dir,
    output: std::process::Output,
}

fn run_command_list(
    workspace: Option<TempDir>,
    entries: &[&str],
    isolation: CommandIsolation,
) -> Result<Option<CommandListRun>> {
    let Some(tempdir) = workspace else {
        return Ok(None);
    };
    let path = Utf8PathBuf::from_path_buf(tempdir.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("temp dir path {} is not UTF-8", path.display()))?;
    let action = Action {
        recipe: Recipe::Command {
            command: StringOrList::List(entries.iter().map(|entry| (*entry).into()).collect()),
            isolation,
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let edge = BuildEdge {
        action_id: "chain".into(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        dependency_order: DependencyOrder::Parallel,
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        progress_label: None,
        phony: false,
        always: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("chain".into(), action);
    graph.insert_edge(edge)?;
    graph.default_targets.push(Utf8PathBuf::from("out"));

    let ninja = generate(&graph)?;
    let dir = Dir::open_ambient_dir(&path, ambient_authority())
        .with_context(|| format!("open ambient dir for temp workspace at {path}"))?;
    dir.write("build.ninja", ninja.as_bytes())
        .context("write ninja build file")?;
    let output = Command::new("ninja")
        .arg("out")
        .current_dir(path.as_std_path())
        .output()
        .context("invoke ninja")?;

    Ok(Some(CommandListRun {
        _workspace: tempdir,
        dir,
        output,
    }))
}

#[rstest]
fn command_list_fails_fast_at_first_nonzero_exit(
    ninja_integration_setup: Option<TempDir>,
) -> Result<()> {
    let Some(run) = run_command_list(
        ninja_integration_setup,
        &["echo one > first.txt", "false", "echo never > last.txt"],
        CommandIsolation::Shared,
    )?
    else {
        return Ok(());
    };
    ensure!(
        !run.output.status.success(),
        "command chain should fail when an entry exits non-zero"
    );
    let diagnostics = format!(
        "{}{}",
        String::from_utf8_lossy(&run.output.stdout),
        String::from_utf8_lossy(&run.output.stderr)
    );
    ensure!(
        diagnostics.contains("netsuke command-list failure:") && diagnostics.contains(", entry 2"),
        "failure should identify the one-based failing entry: {diagnostics}"
    );
    let first = run
        .dir
        .read_to_string("first.txt")
        .context("first entry should have run and written first.txt")?;
    ensure!(
        first.trim() == "one",
        "first entry should have written its output, got '{first}'"
    );
    ensure!(
        !run.dir.try_exists("last.txt").context("check last.txt")?,
        "fail-fast chain should skip entries after the first non-zero exit"
    );
    Ok(())
}

#[rstest]
fn command_list_entry_control_flow_cannot_mask_an_earlier_failure(
    ninja_integration_setup: Option<TempDir>,
) -> Result<()> {
    // Without per-entry isolation, the second entry's `||` would join the raw
    // chain as `false && false || echo recovered > recovered.txt`, causing
    // POSIX to run the echo and report success despite the first failure.
    let Some(run) = run_command_list(
        ninja_integration_setup,
        &["false", "false || echo recovered > recovered.txt"],
        CommandIsolation::Shared,
    )?
    else {
        return Ok(());
    };
    ensure!(
        !run.output.status.success(),
        "the first entry's failure must not be masked by a later '||': {:?}",
        run.output
    );
    ensure!(
        !run.dir
            .try_exists("recovered.txt")
            .context("check recovered.txt")?,
        "the second entry should not run after the first entry fails"
    );
    Ok(())
}

#[rstest]
fn command_list_entries_share_one_shell_process(
    ninja_integration_setup: Option<TempDir>,
) -> Result<()> {
    // Final Ninja serialisation escapes this ordinary dollar, so the shell sees
    // the value written by the first entry.
    let Some(run) = run_command_list(
        ninja_integration_setup,
        &[
            "export NETSUKE_SHARED=yes",
            "test \"$NETSUKE_SHARED\" = yes && echo ok > shared.txt",
        ],
        CommandIsolation::Shared,
    )?
    else {
        return Ok(());
    };
    ensure!(
        run.output.status.success(),
        "command chain should succeed when every entry succeeds: {:?}",
        run.output
    );
    let shared = run
        .dir
        .read_to_string("shared.txt")
        .context("later entries should see the environment set by an earlier entry")?;
    ensure!(
        shared.trim() == "ok",
        "unexpected shared.txt content: {shared}"
    );
    Ok(())
}

#[rstest]
fn shared_command_list_carries_working_directory_between_entries(
    ninja_integration_setup: Option<TempDir>,
) -> Result<()> {
    let Some(run) = run_command_list(
        ninja_integration_setup,
        &["mkdir nested && cd nested", "pwd > shared-cwd.txt"],
        CommandIsolation::Shared,
    )?
    else {
        return Ok(());
    };
    ensure!(
        run.output.status.success(),
        "shared `cd` chain should succeed"
    );
    let nested = run
        .dir
        .read_to_string("nested/shared-cwd.txt")
        .context("later shared entry should write from the changed directory")?;
    ensure!(
        nested.trim_end().ends_with("/nested"),
        "later entry should inherit the changed working directory: {nested}"
    );
    Ok(())
}

#[rstest]
fn isolated_command_list_resets_location_environment_and_shell_variables(
    ninja_integration_setup: Option<TempDir>,
) -> Result<()> {
    let Some(run) = run_command_list(
        ninja_integration_setup,
        &[
            "mkdir nested && cd nested && export NETSUKE_ISOLATED_ENV=leaked && NETSUKE_ISOLATED_VAR=leaked",
            "printf '%s|%s|%s' \"${NETSUKE_ISOLATED_ENV-unset}\" \"${NETSUKE_ISOLATED_VAR-unset}\" \"$PWD\" > isolated-state.txt",
        ],
        CommandIsolation::Isolated,
    )?
    else {
        return Ok(());
    };
    ensure!(
        run.output.status.success(),
        "isolated entries should succeed"
    );
    let state = run
        .dir
        .read_to_string("isolated-state.txt")
        .context("second isolated entry should write in the action directory")?;
    ensure!(
        state.starts_with("unset|unset|"),
        "later entry should not inherit the earlier environment or shell variable: {state}"
    );
    ensure!(
        !state.ends_with("/nested"),
        "later entry should start from the action's working directory: {state}"
    );
    Ok(())
}

#[rstest]
fn isolated_successful_exit_has_no_failure_marker_and_allows_the_next_entry(
    ninja_integration_setup: Option<TempDir>,
) -> Result<()> {
    let Some(run) = run_command_list(
        ninja_integration_setup,
        &["exit 0", "echo continued > continued.txt"],
        CommandIsolation::Isolated,
    )?
    else {
        return Ok(());
    };
    ensure!(
        run.output.status.success(),
        "isolated successful exit should allow later entries: {:?}",
        run.output
    );
    let stderr = String::from_utf8_lossy(&run.output.stderr);
    ensure!(
        !stderr.contains("netsuke command-list failure:"),
        "successful early exit must not leave a failure marker: {stderr}"
    );
    ensure!(
        run.dir.try_exists("continued.txt")?,
        "later isolated entry should run after exit 0"
    );
    Ok(())
}

#[rstest]
fn isolated_direct_exec_is_entry_only_and_allows_later_entries(
    ninja_integration_setup: Option<TempDir>,
) -> Result<()> {
    let Some(run) = run_command_list(
        ninja_integration_setup,
        &[
            "exec sh -c 'echo replaced > replaced.txt'",
            "echo continued > later.txt",
        ],
        CommandIsolation::Isolated,
    )?
    else {
        return Ok(());
    };
    ensure!(
        run.output.status.success(),
        "successful direct exec should succeed"
    );
    ensure!(run.dir.try_exists("replaced.txt")?, "exec child should run");
    ensure!(
        run.dir.try_exists("later.txt")?,
        "successful direct exec should end only its isolated entry"
    );
    Ok(())
}

#[rstest]
fn isolated_command_list_waits_for_its_single_background_job(
    ninja_integration_setup: Option<TempDir>,
) -> Result<()> {
    let Some(run) = run_command_list(
        ninja_integration_setup,
        &[
            "(sleep 0.1 && echo waited > background.txt) &",
            "test -f background.txt && echo later > later.txt",
        ],
        CommandIsolation::Isolated,
    )?
    else {
        return Ok(());
    };
    ensure!(
        run.output.status.success(),
        "isolated background job should complete before the next entry: {:?}",
        run.output
    );
    ensure!(
        run.dir.try_exists("later.txt")?,
        "later isolated entry should run"
    );
    Ok(())
}
