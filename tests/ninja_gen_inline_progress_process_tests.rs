//! Real-Ninja process tests for inline progress descriptions.

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
use test_support::{localizer_test_lock, ninja_gen, set_en_localizer};

/// Supply a temporary workspace when real Ninja is available.
///
/// The fixture returns `None` when the Ninja executable is unavailable.
#[fixture]
fn ninja_integration_setup() -> Option<TempDir> {
    ninja_gen::ninja_integration_setup()
}

/// Own the workspace and the captured output of one real-Ninja invocation.
struct NinjaRun {
    /// Keep the build directory alive until the assertions finish.
    _workspace: TempDir,
    /// Capture Ninja's progress and the recipe's output streams.
    output: std::process::Output,
}

/// Generate a one-edge Ninja file and run the selected output in real Ninja.
///
/// For example, `run_action(workspace, action, Some("check-fmt"))` attaches
/// the raw name that the generator turns into readable progress text.
fn run_action(
    workspace: Option<TempDir>,
    action: Action,
    progress_label: Option<&str>,
) -> Result<Option<NinjaRun>> {
    let Some(tempdir) = workspace else {
        return Ok(None);
    };
    let path = Utf8PathBuf::from_path_buf(tempdir.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("temp dir path {} is not UTF-8", path.display()))?;
    let edge = BuildEdge {
        action_id: "progress-action".into(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        dependency_order: DependencyOrder::Parallel,
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        progress_label: progress_label.map(str::to_owned),
        phony: false,
        always: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert(edge.action_id.clone(), action);
    graph.insert_edge(edge)?;

    let dir = Dir::open_ambient_dir(&path, ambient_authority())
        .with_context(|| format!("open ambient dir for temp workspace at {path}"))?;
    dir.write("build.ninja", generate(&graph)?.as_bytes())
        .context("write generated Ninja file")?;
    let output = Command::new("ninja")
        .arg("out")
        .current_dir(path.as_std_path())
        .output()
        .context("invoke ninja")?;
    Ok(Some(NinjaRun {
        _workspace: tempdir,
        output,
    }))
}

/// Build a command action with the supplied recipe and optional rule wording.
///
/// For example, `command_action("echo ready".into(), None)` builds an inline
/// command action without named-rule metadata.
fn command_action(command: StringOrList, description: Option<&str>) -> Action {
    Action {
        recipe: Recipe::Command {
            command,
            isolation: CommandIsolation::Shared,
        },
        description: description.map(str::to_owned),
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    }
}

/// Join captured output streams for progress-line assertions.
///
/// For example, `output_text(&run)` gives one searchable view of Ninja output.
fn output_text(run: &NinjaRun) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&run.output.stdout),
        String::from_utf8_lossy(&run.output.stderr)
    )
}

/// Return the first task-progress line emitted by this one-edge build.
///
/// For example, `[1/1] Building check-fmt` is returned as one line.
fn progress_line(output: &str) -> Result<&str> {
    output
        .lines()
        .find(|line| line.starts_with("[1/1]"))
        .context("expected one Ninja progress line")
}

#[rstest]
fn scalar_inline_recipe_prints_its_readable_name(
    ninja_integration_setup: Option<TempDir>,
) -> Result<()> {
    let _lock = localizer_test_lock().map_err(|error| anyhow::anyhow!("{error}"))?;
    let _localizer = set_en_localizer();
    let Some(run) = run_action(
        ninja_integration_setup,
        command_action(StringOrList::String("printf done".into()), None),
        Some("check-fmt"),
    )?
    else {
        return Ok(());
    };
    ensure!(run.output.status.success(), "Ninja build should succeed");
    let output = output_text(&run);
    let line = progress_line(&output)?;
    ensure!(
        line.contains("Building check-fmt"),
        "scalar progress should use its readable name: {line}"
    );
    ensure!(
        !line.contains("_netsuke_background_before"),
        "progress should not expose the generated wrapper: {line}"
    );
    Ok(())
}

#[rstest]
fn command_list_progress_keeps_tool_stdout_in_declaration_order(
    ninja_integration_setup: Option<TempDir>,
) -> Result<()> {
    let _lock = localizer_test_lock().map_err(|error| anyhow::anyhow!("{error}"))?;
    let _localizer = set_en_localizer();
    let Some(run) = run_action(
        ninja_integration_setup,
        command_action(
            StringOrList::List(vec![
                "printf 'first-tool-output\\n'".into(),
                "printf 'second-tool-output\\n'".into(),
            ]),
            None,
        ),
        Some("check-fmt"),
    )?
    else {
        return Ok(());
    };
    ensure!(run.output.status.success(), "Ninja build should succeed");
    let stdout = String::from_utf8_lossy(&run.output.stdout);
    let line = progress_line(&stdout)?;
    ensure!(
        line.contains("Building check-fmt (2 commands)"),
        "list progress should include the localized entry count: {line}"
    );
    ensure!(
        !line.contains("_netsuke_background_before"),
        "progress should not expose the generated wrapper: {line}"
    );
    let first = stdout
        .find("first-tool-output")
        .context("expected first tool output")?;
    let second = stdout
        .find("second-tool-output")
        .context("expected second tool output")?;
    ensure!(
        first < second,
        "command-list stdout should retain declaration order: {stdout}"
    );
    Ok(())
}

#[rstest]
fn named_rule_description_remains_authoritative(
    ninja_integration_setup: Option<TempDir>,
) -> Result<()> {
    let Some(run) = run_action(
        ninja_integration_setup,
        command_action(
            StringOrList::String("printf done".into()),
            Some("Compile with the named rule"),
        ),
        None,
    )?
    else {
        return Ok(());
    };
    ensure!(run.output.status.success(), "Ninja build should succeed");
    let output = output_text(&run);
    let line = progress_line(&output)?;
    ensure!(
        line.contains("Compile with the named rule") && !line.contains("Building out"),
        "named-rule progress should remain authoritative: {line}"
    );
    Ok(())
}
