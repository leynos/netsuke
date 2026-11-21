//! Real-Ninja coverage for a depfile the consumer's own recipe writes.
//!
//! A `phony` edge's recipe runs against a hidden output, but the recipe can
//! only name its declared output: `{{ outs }}` is lowered from the declared
//! names, and `$out` is a literal shell variable. A depfile written against
//! that public name therefore never matches the hidden output Makefile-style
//! loading compares against, while a `deps` binding records the same depfile's
//! inputs in Ninja's dependency log under the hidden output. These cases pin
//! both outcomes; the crate root covers the depfile that is already present
//! when the first build scans the graph.

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use netsuke::ir::{BuildEdge, BuildGraph, DependencyOrder, EdgeOutputVisibility};
use netsuke::ninja_gen::{RecipeShell, generate_with_shell};
use rstest::rstest;
use std::process::Command;
use tempfile::TempDir;
use test_support::ninja::resolve_ninja_program;

use super::{
    DEPFILE, GENERATED, RECIPE_PATH, RESULT, RUNS_LOG, action, hidden_output, insert_edge, run_log,
    run_ninja, temp_workspace, write_workspace,
};

/// Build an always-dirty producer and a consumer whose recipe writes a depfile.
///
/// The consumer names [`RESULT`] as the depfile's target, which can never equal
/// the hidden output. Nothing pre-loads the entry, so only Ninja's own
/// dependency log can carry the discovered input to a later build. The producer
/// is not a declared dependency and re-runs on every invocation, so a later
/// build can order it before the consumer only through that recorded entry.
fn recipe_depfile_graph(deps: bool) -> BuildGraph {
    let mut graph = BuildGraph::default();
    graph.actions.insert(
        "produce".to_owned(),
        action(
            &format!("printf 'produce\\n' >> {RUNS_LOG} && printf 'header\\n' > {GENERATED}"),
            None,
            None,
        ),
    );
    graph.actions.insert(
        "consume".to_owned(),
        action(
            &format!("printf 'consume\\n' >> {RUNS_LOG} && printf 'out\\n' > {RESULT} && printf '{RESULT}: {GENERATED}\\n' > {DEPFILE}"),
            Some(DEPFILE),
            deps.then_some("gcc"),
        ),
    );
    graph
        .insert_edge(BuildEdge {
            action_id: "produce".to_owned(),
            inputs: Vec::new(),
            implicit_deps: Vec::new(),
            dependency_order: DependencyOrder::Parallel,
            explicit_outputs: vec![Utf8PathBuf::from(GENERATED)],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            progress_label: None,
            phony: false,
            always: true,
            output_visibility: EdgeOutputVisibility::default(),
        })
        .expect("fixture outputs are unique");
    insert_edge(&mut graph, "consume", RESULT, true);
    graph.default_targets.push(Utf8PathBuf::from(RESULT));
    graph
}

/// Run one Ninja tool subcommand over the workspace and return its output.
fn run_ninja_tool(directory: &Utf8Path, tool: &str) -> Result<String> {
    let ninja =
        resolve_ninja_program().context("ninja must be on PATH for the depfile runtime tests")?;
    let output = Command::new(ninja)
        .args(["-f", "build.ninja", "-t", tool])
        .current_dir(directory.as_std_path())
        .env_clear()
        .env("PATH", RECIPE_PATH)
        .output()
        .with_context(|| format!("run ninja -t {tool} in {directory}"))?;
    ensure!(
        output.status.success(),
        "ninja -t {tool} must succeed: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// A recipe-written depfile with `deps` must order the producer next time.
///
/// The recipe names its public output as the depfile's target, which can never
/// equal the hidden output, so Makefile-style loading would refuse it. Ninja's
/// dependency log keys the entry by the hidden output instead, so the second
/// build orders the always-dirty producer before the recipe.
#[rstest]
fn a_recipe_written_depfile_orders_the_producer_on_a_later_build(
    temp_workspace: Result<Option<TempDir>>,
) -> Result<()> {
    let Some(directory) = temp_workspace? else {
        return Ok(());
    };
    let ninja = generate_with_shell(&recipe_depfile_graph(true), RecipeShell::Posix)
        .context("generate the recipe-depfile graph")?;
    let hidden = hidden_output(&ninja)?;
    let (root, path) = write_workspace(&directory, &ninja)?;

    let first = run_ninja(&path)?;
    ensure!(
        first.status.success(),
        "the first build must succeed: {}{}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr),
    );
    let deps = run_ninja_tool(&path, "deps")?;
    ensure!(
        deps.contains(hidden.as_str()) && deps.contains(GENERATED),
        "the dependency log must record the input against the hidden output: {deps}",
    );

    let second = run_ninja(&path)?;
    ensure!(
        second.status.success(),
        "the second build must succeed: {}{}",
        String::from_utf8_lossy(&second.stdout),
        String::from_utf8_lossy(&second.stderr),
    );
    let log = run_log(&root)?;
    ensure!(
        log.ends_with(&["produce".to_owned(), "consume".to_owned()]),
        "the recorded input must order the producer before the recipe: {log:?}",
    );
    Ok(())
}

/// The same recipe-written depfile without `deps` must record nothing.
///
/// Makefile-style loading compares the depfile's target with the edge's first
/// output and refuses the mismatch, so the producer stays unreachable and the
/// recipe runs alone on both builds.
#[rstest]
fn without_deps_a_recipe_written_depfile_orders_nothing(
    temp_workspace: Result<Option<TempDir>>,
) -> Result<()> {
    let Some(directory) = temp_workspace? else {
        return Ok(());
    };
    let ninja = generate_with_shell(&recipe_depfile_graph(false), RecipeShell::Posix)
        .context("generate the recipe-depfile graph")?;
    let (root, path) = write_workspace(&directory, &ninja)?;

    for _ in 0..2 {
        let output = run_ninja(&path)?;
        ensure!(
            output.status.success(),
            "the recipe must run without the discovered input: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
    let deps = run_ninja_tool(&path, "deps")?;
    ensure!(
        !deps.contains(GENERATED),
        "a refused depfile must record no input: {deps}",
    );
    let log = run_log(&root)?;
    ensure!(
        !log.iter().any(|entry| entry == "produce"),
        "the producer must never run: {log:?}",
    );
    Ok(())
}
