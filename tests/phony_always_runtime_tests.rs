//! Real-Ninja runtime tests for the `phony` and `always` lowerings.
//!
//! Every scenario here drives the `netsuke` CLI against an actual `ninja`
//! binary and asserts observable effects: lines appended to a log, files that
//! survive `netsuke clean`, directories a recipe writes into, and files that do
//! not. The generated text is pinned separately by
//! `tests/ninja_phony_lowering_tests.rs`; this file exists because
//! correct-looking text is not evidence that Ninja schedules it the way the
//! manifest author asked.
//!
//! Workspace and process setup live in [`phony_runtime`], so each scenario
//! reads as the behaviour it checks. The child process receives its own `PATH`
//! and `NETSUKE_NINJA`, so no in-process environment mutation is involved even
//! though the scenarios run in parallel.

#![cfg(unix)]

mod phony_runtime;

use anyhow::{Result, ensure};
use camino::Utf8PathBuf;
use phony_runtime::{
    ACTION_MANIFEST, ALWAYS_MANIFEST, CLEAN_MANIFEST, FORCE_NODE, NESTED_OUTPUT_MANIFEST,
    PHONY_TARGET_MANIFEST, PREREQUISITE_DIRECTORY_MANIFEST, Shadow, UNSELECTED_OUTPUT_MANIFEST,
    Workspace, alias_manifest, temp_workspace,
};
use rstest::rstest;
use tempfile::TempDir;

/// Run a logical target despite a same-named user path, on every build.
///
/// The user path must survive, because the hidden statement never claims it and
/// the alias that publishes the declared name is never materialized.
#[rstest]
#[case::action_over_a_directory(ACTION_MANIFEST, Shadow::Directory)]
#[case::action_over_a_file(ACTION_MANIFEST, Shadow::File)]
#[case::phony_target_over_a_directory(PHONY_TARGET_MANIFEST, Shadow::Directory)]
#[case::phony_target_over_a_file(PHONY_TARGET_MANIFEST, Shadow::File)]
fn a_logical_target_runs_despite_a_same_named_user_path(
    temp_workspace: Result<Option<TempDir>>,
    #[case] manifest: &str,
    #[case] shadow: Shadow,
) -> Result<()> {
    let Some(directory) = temp_workspace? else {
        return Ok(());
    };
    let workspace = Workspace::new(directory)?;
    workspace.write_manifest(manifest)?;
    shadow.create(&workspace, "check")?;

    workspace.build()?;
    ensure!(
        workspace.lines("runs.log")?.len() == 1,
        "a same-named user path must not suppress the recipe: {:?}",
        workspace.lines("runs.log")?,
    );

    workspace.build()?;
    ensure!(
        workspace.lines("runs.log")?.len() == 2,
        "a logical target must run on every build: {:?}",
        workspace.lines("runs.log")?,
    );
    ensure!(
        workspace.exists(shadow.survivor("check").as_str())?,
        "the build must not consume the user path that shares the target's name",
    );
    Ok(())
}

/// Let a phony recipe write an output whose directory it did not create.
///
/// The hidden statement names only `.netsuke/phony/edge-*`, so Ninja prepares
/// that namespace and never `build/`. The generated preparation edge has to
/// create `build/` before the recipe runs, on a clean workspace and after the
/// directory has been deleted.
#[rstest]
fn a_nested_phony_output_finds_its_directory(
    temp_workspace: Result<Option<TempDir>>,
) -> Result<()> {
    let Some(directory) = temp_workspace? else {
        return Ok(());
    };
    let workspace = Workspace::new(directory)?;
    workspace.write_manifest(NESTED_OUTPUT_MANIFEST)?;

    workspace.build()?;
    ensure!(
        workspace.exists("build/report")?,
        "a phony recipe must be able to write an output whose directory it did not create",
    );
    let report = workspace.read_to_string("build/report")?;
    ensure!(
        report.trim() == "report",
        "the recipe must write the declared output: {report:?}",
    );
    Ok(())
}

/// Recreate a deleted output directory on the next build.
///
/// The preparation node is never created, so Ninja reruns its idempotent
/// command whenever the recipe statement is selected. That is what lets a
/// directory removed since the last build come back instead of failing the
/// recipe.
#[rstest]
fn a_deleted_output_directory_is_recreated(temp_workspace: Result<Option<TempDir>>) -> Result<()> {
    let Some(directory) = temp_workspace? else {
        return Ok(());
    };
    let workspace = Workspace::new(directory)?;
    workspace.write_manifest(NESTED_OUTPUT_MANIFEST)?;
    workspace.build()?;
    workspace.remove_dir_all("build")?;

    workspace.build()?;
    ensure!(
        workspace.exists("build/report")?,
        "the preparation edge must recreate a directory deleted since the last build",
    );
    Ok(())
}

/// Leave an unselected phony edge's output directory alone.
///
/// Preparation runs as part of the selected recipe, so a nested phony output
/// that nobody asked for creates nothing. Only the declared-name build request
/// reaches that edge's preparation.
#[rstest]
fn an_unselected_phony_output_creates_no_directory(
    temp_workspace: Result<Option<TempDir>>,
) -> Result<()> {
    let Some(directory) = temp_workspace? else {
        return Ok(());
    };
    let workspace = Workspace::new(directory)?;
    workspace.write_manifest(UNSELECTED_OUTPUT_MANIFEST)?;

    workspace.build()?;
    ensure!(
        workspace.exists("other.log")?,
        "the selected default target must run",
    );
    ensure!(
        !workspace.exists("build")?,
        "an unselected phony edge must not create its output directory",
    );

    let run = workspace.run(&["build", "build/report"])?;
    ensure!(
        run.success,
        "building the nested output failed: {}",
        run.stderr
    );
    ensure!(
        workspace.exists("build/report")?,
        "selecting the edge must prepare its directory and run the recipe",
    );
    Ok(())
}

/// Run preparation after a prerequisite that creates the directory itself.
///
/// Ninja does not order an order-only dependency after the edge's normal
/// inputs, so preparation that only used `||` would run `mkdir -p` before a
/// prerequisite's plain `mkdir` and break it. The preparation edge therefore
/// takes the recipe's prerequisites as its own inputs.
#[rstest]
fn a_prerequisite_that_creates_the_directory_runs_first(
    temp_workspace: Result<Option<TempDir>>,
) -> Result<()> {
    let Some(directory) = temp_workspace? else {
        return Ok(());
    };
    let workspace = Workspace::new(directory)?;
    workspace.write_manifest(PREREQUISITE_DIRECTORY_MANIFEST)?;

    workspace.build()?;
    ensure!(
        workspace.exists("build/seed")?,
        "the prerequisite must create the directory before preparation runs",
    );
    ensure!(
        workspace.exists("build/report")?,
        "the recipe must write its output after the prerequisite and preparation",
    );
    Ok(())
}

/// Re-run an `always` target while `restat` prunes its downstream target.
///
/// The forced target's output keeps its modification time, so the recorded
/// timestamp unchanged is what must stop the consumer from re-running.
#[rstest]
fn always_reruns_and_restat_prunes_its_downstream(
    temp_workspace: Result<Option<TempDir>>,
) -> Result<()> {
    let Some(directory) = temp_workspace? else {
        return Ok(());
    };
    let workspace = Workspace::new(directory)?;
    workspace.write_manifest(ALWAYS_MANIFEST)?;

    workspace.build()?;
    ensure!(
        workspace.lines("runs.log")?.len() == 1,
        "the forced target must run on the first build: {:?}",
        workspace.lines("runs.log")?,
    );
    ensure!(
        workspace.lines("consumer.log")?.len() == 1,
        "the downstream target must run on the first build: {:?}",
        workspace.lines("consumer.log")?,
    );

    workspace.build()?;
    ensure!(
        workspace.lines("runs.log")?.len() == 2,
        "an always target must re-run while its inputs are unchanged: {:?}",
        workspace.lines("runs.log")?,
    );
    ensure!(
        workspace.lines("consumer.log")?.len() == 1,
        "restat must prune the downstream target when the forced output is unchanged: {:?}",
        workspace.lines("consumer.log")?,
    );
    Ok(())
}

/// Clear a stray path at the force node before Ninja runs.
///
/// Ninja reads the node's absence as permanently dirty, so a leftover file
/// would silently turn the `always` lowering off.
#[rstest]
fn a_stray_force_node_is_cleared_before_the_build(
    temp_workspace: Result<Option<TempDir>>,
) -> Result<()> {
    let Some(directory) = temp_workspace? else {
        return Ok(());
    };
    let workspace = Workspace::new(directory)?;
    workspace.write_manifest(ALWAYS_MANIFEST)?;
    workspace.create_dir_all(".netsuke/phony")?;
    workspace.write(FORCE_NODE, b"stray")?;

    workspace.build()?;
    ensure!(
        !workspace.exists(FORCE_NODE)?,
        "a stray path at the force node must not survive the build",
    );

    workspace.build()?;
    ensure!(
        workspace.lines("runs.log")?.len() == 2,
        "a stray force node must not suppress the always target: {:?}",
        workspace.lines("runs.log")?,
    );
    Ok(())
}

/// Refuse a manifest that declares the force node under an absolute spelling.
///
/// The lexical generation check cannot judge such a spelling, so the build
/// command resolves it against the working directory and stops before it
/// removes the declared output.
#[rstest]
fn a_declared_alias_of_the_force_node_stops_the_build(
    temp_workspace: Result<Option<TempDir>>,
) -> Result<()> {
    let Some(directory) = temp_workspace? else {
        return Ok(());
    };
    let force_node = Utf8PathBuf::from_path_buf(directory.path().join(FORCE_NODE))
        .map_err(|path| anyhow::anyhow!("force node path is not UTF-8: {}", path.display()))?;
    let workspace = Workspace::new(directory)?;
    workspace.write_manifest(&alias_manifest(force_node.as_str()))?;
    workspace.create_dir_all(".netsuke/phony")?;
    workspace.write(FORCE_NODE, b"declared output")?;

    let run = workspace.run(&["build"])?;
    ensure!(
        !run.success,
        "a manifest declaring the force node under an absolute spelling must be refused",
    );
    ensure!(
        run.stderr.contains("reserved"),
        "the refusal must report the reservation, not a Ninja failure: {}",
        run.stderr,
    );
    ensure!(
        workspace.exists(FORCE_NODE)?,
        "the refused build must leave the declared output in place",
    );
    Ok(())
}

/// Refuse the alias on a clean workspace, before any generated state exists.
///
/// The refusal must not depend on the force node being present, and it must not
/// create the reserved namespace while deciding.
#[rstest]
fn a_declared_alias_is_refused_before_generated_state_exists(
    temp_workspace: Result<Option<TempDir>>,
) -> Result<()> {
    let Some(directory) = temp_workspace? else {
        return Ok(());
    };
    let alias = Utf8PathBuf::from_path_buf(directory.path().join(FORCE_NODE).join("report"))
        .map_err(|path| anyhow::anyhow!("alias path is not UTF-8: {}", path.display()))?;
    let workspace = Workspace::new(directory)?;
    workspace.write_manifest(&alias_manifest(alias.as_str()))?;

    let run = workspace.run(&["build"])?;
    ensure!(
        !run.success,
        "an absolute alias must be refused before the namespace exists",
    );
    ensure!(
        !workspace.exists(".netsuke")?,
        "a refused build must not create the reserved namespace",
    );
    Ok(())
}

/// Preserve user paths and remove real outputs when cleaning.
///
/// Ninja's `clean` tool skips phony outputs, so a user path sharing a phony
/// target's name survives while a real `always` output is removed.
#[rstest]
fn clean_preserves_user_paths_and_removes_real_outputs(
    temp_workspace: Result<Option<TempDir>>,
) -> Result<()> {
    let Some(directory) = temp_workspace? else {
        return Ok(());
    };
    let workspace = Workspace::new(directory)?;
    workspace.write_manifest(CLEAN_MANIFEST)?;
    Shadow::Directory.create(&workspace, "check")?;

    workspace.build()?;
    ensure!(
        workspace.exists("stamp")?,
        "the always target must materialise its declared output",
    );
    ensure!(
        workspace.lines("runs.log")?.len() == 2,
        "both the action and the always target must run: {:?}",
        workspace.lines("runs.log")?,
    );

    let clean = workspace.run(&["clean"])?;
    ensure!(clean.success, "netsuke clean failed: {}", clean.stderr);

    ensure!(
        workspace.exists("check/keep.txt")?,
        "clean must not remove a user path that shares a phony name",
    );
    ensure!(
        !workspace.exists("stamp")?,
        "clean must remove the output of a real always edge",
    );
    Ok(())
}
