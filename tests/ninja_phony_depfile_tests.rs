//! Real-Ninja coverage for a depfile on an executable `phony` edge.
//!
//! A `phony` edge's recipe runs against a hidden output, so a Makefile-style
//! depfile — a `depfile` binding without `deps` — must name that hidden path
//! as its first target. No recipe can name it: `{{ outs }}` is lowered from
//! the declared names, and `$out` is a literal shell variable. This suite pins
//! each outcome that boundary produces:
//!
//! - A pre-written depfile naming the hidden output is loaded at scan time and
//!   orders the discovered producer before the recipe.
//! - A depfile the recipe writes against its public name is refused by
//!   Makefile-style loading, so its producer never runs.
//! - With `deps`, Ninja records that same recipe-written depfile's inputs
//!   against the hidden output, and the next build orders the producer first.
//!
//! The graphs are constructed programmatically because `Action.depfile` and
//! `Action.deps_format` are always `None` from the manifest loader; only
//! direct IR construction can populate them.
//!
//! When Ninja is unavailable the scenarios skip, unless the injected
//! `NETSUKE_REQUIRE_NINJA=1` setting has made a missing installation fatal.

#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use netsuke::ast::{CommandIsolation, Recipe, StringOrList};
use netsuke::ir::{Action, BuildEdge, BuildGraph, DependencyOrder, EdgeOutputVisibility};
use netsuke::ninja_gen::{RecipeShell, generate_with_shell};
use rstest::{fixture, rstest};
use std::process::{Command, Output};
use tempfile::TempDir;
use test_support::ninja::{
    NinjaWorkspaceError, ninja_integration_workspace, resolve_ninja_program,
};

/// The `PATH` recipes see: a host utility directory, not the test runner's.
const RECIPE_PATH: &str = "/usr/bin:/bin";

/// Prefix of the hidden output that carries the consumer's recipe.
const HIDDEN_PREFIX: &str = ".netsuke/phony/edge-";

/// The producer's declared output, reachable only through the depfile.
const GENERATED: &str = "generated.h";

/// The consumer's declared output, and the default target of the build.
const RESULT: &str = "result.txt";

/// Where both recipes record the order they ran in.
const RUNS_LOG: &str = "runs.log";

/// The depfile the consumer's rule names.
const DEPFILE: &str = "deps.d";

/// Provide a temporary workspace, or skip when Ninja is unavailable.
///
/// Only a failed Ninja probe is a skip: a probe answer means the scenario was
/// expected to run, so failing to create its workspace is a real error rather
/// than an environment gap.
#[fixture]
fn temp_workspace() -> Result<Option<TempDir>> {
    match ninja_integration_workspace() {
        Ok(directory) => Ok(Some(directory)),
        Err(
            NinjaWorkspaceError::ProbeSpawn(_)
            | NinjaWorkspaceError::ProbeFailed(_)
            | NinjaWorkspaceError::ProbeTimeout(_),
        ) => Ok(None),
        Err(error @ NinjaWorkspaceError::Workspace(_)) => {
            Err(error).context("create the Ninja integration workspace")
        }
    }
}

/// Build an action whose recipe is `command` and whose rule names `depfile`.
fn action(command: &str, depfile: Option<&str>, deps: Option<&str>) -> Action {
    Action {
        recipe: Recipe::Command {
            command: StringOrList::String(command.to_owned()),
            isolation: CommandIsolation::Shared,
        },
        description: None,
        depfile: depfile.map(str::to_owned),
        deps_format: deps.map(str::to_owned),
        pool: None,
        restat: false,
    }
}

/// Insert one edge naming a single explicit output.
fn insert_edge(graph: &mut BuildGraph, action_id: &str, output: &str, phony: bool) {
    graph
        .insert_edge(BuildEdge {
            action_id: action_id.to_owned(),
            inputs: Vec::new(),
            implicit_deps: Vec::new(),
            dependency_order: DependencyOrder::Parallel,
            explicit_outputs: vec![Utf8PathBuf::from(output)],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            progress_label: None,
            phony,
            always: false,
            output_visibility: EdgeOutputVisibility::default(),
        })
        .expect("fixture outputs are unique");
}

/// Build the producer and the depfile-carrying `phony` consumer.
///
/// The producer is deliberately not a declared dependency of the consumer and
/// not a default target: the only path from [`RESULT`] to [`GENERATED`] is the
/// depfile Ninja loads.
fn depfile_graph() -> BuildGraph {
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
            &format!("printf 'consume\\n' >> {RUNS_LOG} && cat {GENERATED} > {RESULT}"),
            Some(DEPFILE),
            None,
        ),
    );
    insert_edge(&mut graph, "produce", GENERATED, false);
    insert_edge(&mut graph, "consume", RESULT, true);
    graph.default_targets.push(Utf8PathBuf::from(RESULT));
    graph
}

/// Return the hidden output the generated text gives the consumer's recipe.
fn hidden_output(ninja: &str) -> Result<Utf8PathBuf> {
    ninja
        .lines()
        .filter_map(|line| line.strip_prefix("build "))
        .filter_map(|statement| statement.split_once(": ").map(|(outputs, _)| outputs))
        .find(|outputs| outputs.starts_with(HIDDEN_PREFIX))
        .map(Utf8PathBuf::from)
        .with_context(|| format!("the generated text must name one hidden output:\n{ninja}"))
}

/// Run the real Ninja over the generated `build.ninja` in `directory`.
fn run_ninja(directory: &Utf8Path) -> Result<Output> {
    let ninja =
        resolve_ninja_program().context("ninja must be on PATH for the depfile runtime tests")?;
    Command::new(ninja)
        .args(["-f", "build.ninja"])
        .current_dir(directory.as_std_path())
        .env_clear()
        .env("PATH", RECIPE_PATH)
        .output()
        .with_context(|| format!("run ninja in {directory}"))
}

/// Read the run log as lines, treating a missing file as no lines.
fn run_log(root: &Dir) -> Result<Vec<String>> {
    let exists = root
        .try_exists(RUNS_LOG)
        .with_context(|| format!("check whether {RUNS_LOG} exists"))?;
    if !exists {
        return Ok(Vec::new());
    }
    let content = root
        .read_to_string(RUNS_LOG)
        .with_context(|| format!("read {RUNS_LOG}"))?;
    Ok(content.lines().map(str::to_owned).collect())
}

/// Write the generated text into the temporary workspace.
fn write_workspace(directory: &TempDir, ninja: &str) -> Result<(Dir, Utf8PathBuf)> {
    let path = Utf8Path::from_path(directory.path())
        .context("the temporary workspace path must be UTF-8")?;
    let root =
        Dir::open_ambient_dir(path, ambient_authority()).context("open the Ninja workspace")?;
    root.write("build.ninja", ninja)
        .context("write the generated Ninja file")?;
    Ok((root, Utf8PathBuf::from(path)))
}

/// Write the generated text and, when asked, the depfile naming the hidden output.
fn prepare_workspace(directory: &TempDir, load_depfile: bool) -> Result<(Dir, Utf8PathBuf)> {
    let ninja = generate_with_shell(&depfile_graph(), RecipeShell::Posix)
        .context("generate the depfile graph")?;
    let hidden = hidden_output(&ninja)?;
    let (root, path) = write_workspace(directory, &ninja)?;
    if load_depfile {
        root.write(DEPFILE, format!("{hidden}: {GENERATED}\n"))
            .context("write the depfile naming the hidden output")?;
    }
    Ok((root, path))
}

/// A loaded depfile must order the discovered producer before the phony recipe.
///
/// Ninja treats a rule-level `depfile` without `deps` as Makefile-style
/// dependencies it reads at startup. The consumer names [`GENERATED`] nowhere
/// and no declared dependency reaches the producer, so the producer can only
/// run if Ninja loaded the depfile against the hidden output's own name.
#[rstest]
fn a_loaded_depfile_orders_the_discovered_producer_first(
    temp_workspace: Result<Option<TempDir>>,
) -> Result<()> {
    let Some(directory) = temp_workspace? else {
        return Ok(());
    };
    let (root, path) = prepare_workspace(&directory, true)?;

    let output = run_ninja(&path)?;
    ensure!(
        output.status.success(),
        "a depfile naming the hidden output must let the build succeed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    ensure!(
        run_log(&root)? == ["produce", "consume"],
        "the discovered producer must run before the phony recipe: {:?}",
        run_log(&root)?,
    );
    let result = root
        .read_to_string(RESULT)
        .context("read the consumer's output")?;
    ensure!(
        result.trim() == "header",
        "the consumer must read what the producer wrote: {result:?}",
    );
    Ok(())
}

/// The same build without a depfile must never schedule the producer.
///
/// This is the control for the assertion above: with nothing loading the
/// discovered input, Ninja knows no edge reaches the producer, so it runs only
/// the consumer, which fails on the missing header. An implementation that
/// stopped honouring the depfile would produce exactly this outcome.
#[rstest]
fn without_a_loaded_depfile_the_producer_never_runs(
    temp_workspace: Result<Option<TempDir>>,
) -> Result<()> {
    let Some(directory) = temp_workspace? else {
        return Ok(());
    };
    let (root, path) = prepare_workspace(&directory, false)?;

    let output = run_ninja(&path)?;
    ensure!(
        !output.status.success(),
        "a build without a depfile must leave the consumer without its header: {}",
        String::from_utf8_lossy(&output.stdout),
    );
    ensure!(
        run_log(&root)? == ["consume"],
        "nothing may schedule the producer without the depfile: {:?}",
        run_log(&root)?,
    );
    Ok(())
}

/// Cover the depfile the consumer's recipe writes, with and without `deps`.
#[path = "ninja_phony_depfile_tests/recipe_written.rs"]
mod recipe_written;
