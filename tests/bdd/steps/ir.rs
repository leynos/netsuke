//! Step definitions for BuildGraph (IR) scenarios.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::helpers::parse_store::store_parse_outcome;
use anyhow::{Context, Result, anyhow, ensure};
use netsuke::ir::BuildGraph;
use rstest_bdd_macros::{given, then, when};

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Check that IR generation was attempted (either graph or error present).
fn assert_generation_attempted(world: &TestWorld) -> Result<()> {
    match (
        world.build_graph.is_some(),
        world.generation_error.is_filled(),
    ) {
        (true, false) | (false, true) => Ok(()),
        (true, true) => Err(anyhow!("unexpected: graph and error both present")),
        (false, false) => Err(anyhow!("IR generation not attempted")),
    }
}

/// Assert that a BuildGraph collection has the expected count.
fn assert_graph_collection_count<F>(
    world: &TestWorld,
    expected: usize,
    accessor: F,
    field_name: &str,
) -> Result<()>
where
    F: FnOnce(&BuildGraph) -> usize,
{
    let actual = world
        .build_graph
        .with_ref(accessor)
        .context("build graph should be available")?;
    ensure!(
        actual == expected,
        "expected {expected} {field_name}, found {actual}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Given/When steps
// ---------------------------------------------------------------------------

#[given("a new BuildGraph is created")]
#[when("a new BuildGraph is created")]
fn create_graph(world: &TestWorld) -> Result<()> {
    world.build_graph.set_value(BuildGraph::default());
    Ok(())
}

#[given("the manifest file {path:string} is compiled to IR")]
#[when("the manifest file {path:string} is compiled to IR")]
fn compile_manifest(world: &TestWorld, path: &str) -> Result<()> {
    compile_manifest_impl(world, path)
}

#[when("its contents are checked")]
#[when("the graph contents are checked")]
fn graph_checked(world: &TestWorld) -> Result<()> {
    ensure!(
        world.build_graph.is_some(),
        "build graph should be available"
    );
    Ok(())
}

#[when("the generation result is checked")]
fn generation_result_checked(world: &TestWorld) -> Result<()> {
    assert_generation_attempted(world)
}

#[when("an action is removed from the graph")]
fn remove_action(world: &TestWorld) -> Result<()> {
    let mut graph = world
        .build_graph
        .take_value()
        .ok_or_else(|| anyhow!("build graph should be available"))?;

    let first_action = graph.targets.values().next().map(|e| e.action_id.clone());

    if let Some(id) = first_action {
        graph.actions.remove(&id);
        world.removed_action_id.set(id);
        world.build_graph.set_value(graph);
        return Ok(());
    }
    world.build_graph.set_value(graph);
    Err(anyhow!("no actions available to remove"))
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("the graph has {count:usize} actions")]
fn graph_actions(world: &TestWorld, count: usize) -> Result<()> {
    assert_graph_collection_count(world, count, |g| g.actions.len(), "actions")
}

#[then("the graph has {count:usize} targets")]
fn graph_targets(world: &TestWorld, count: usize) -> Result<()> {
    assert_graph_collection_count(world, count, |g| g.targets.len(), "targets")
}

#[then("the graph has {count:usize} default targets")]
fn graph_defaults(world: &TestWorld, count: usize) -> Result<()> {
    assert_graph_collection_count(world, count, |g| g.default_targets.len(), "default targets")
}

#[then("IR generation fails")]
fn ir_generation_fails(world: &TestWorld) -> Result<()> {
    ensure!(
        world.generation_error.is_filled(),
        "expected IR generation error, but none present"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Implementation helpers
// ---------------------------------------------------------------------------

/// Compile a manifest file to IR, storing result or error in state.
fn compile_manifest_impl(world: &TestWorld, path: &str) -> Result<()> {
    let outcome = netsuke::manifest::from_path(path)
        .and_then(|m| BuildGraph::from_manifest(&m).context("building IR from manifest"))
        .with_context(|| format!("IR generation failed for {path}"))
        .map_err(|e| e.to_string());
    store_parse_outcome(&world.build_graph, &world.generation_error, outcome);
    Ok(())
}
