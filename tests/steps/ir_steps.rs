//! Step definitions for `BuildGraph` scenarios.

use crate::CliWorld;
use anyhow::Context;
use cucumber::{given, then, when};
use netsuke::ir::BuildGraph;

fn assert_graph(world: &CliWorld) {
    assert!(
        world.build_graph.is_some(),
        "build graph should have been generated",
    );
}

fn assert_generation_attempted(world: &CliWorld) {
    match (world.build_graph.is_some(), world.manifest_error.is_some()) {
        (true, false) | (false, true) => (),
        (true, true) => panic!("unexpected: graph and error present"),
        (false, false) => panic!("IR generation not attempted"),
    }
}

#[given("a new BuildGraph is created")]
fn create_graph(world: &mut CliWorld) {
    world.build_graph = Some(BuildGraph::default());
}

#[then(expr = "the graph has {int} actions")]
fn graph_actions(world: &mut CliWorld, count: usize) {
    let g = world.build_graph.as_ref().expect("graph");
    assert_eq!(g.actions.len(), count);
}

#[then(expr = "the graph has {int} targets")]
fn graph_targets(world: &mut CliWorld, count: usize) {
    let g = world.build_graph.as_ref().expect("graph");
    assert_eq!(g.targets.len(), count);
}

#[then(expr = "the graph has {int} default targets")]
fn graph_defaults(world: &mut CliWorld, count: usize) {
    let g = world.build_graph.as_ref().expect("graph");
    assert_eq!(g.default_targets.len(), count);
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[given(expr = "the manifest file {string} is compiled to IR")]
#[when(expr = "the manifest file {string} is compiled to IR")]
fn compile_manifest(world: &mut CliWorld, path: String) {
    match netsuke::manifest::from_path(&path)
        .and_then(|m| BuildGraph::from_manifest(&m).context("building IR from manifest"))
        .with_context(|| format!("IR generation failed for {path}"))
    {
        Ok(graph) => {
            world.build_graph = Some(graph);
            world.manifest_error = None;
        }
        Err(e) => {
            world.build_graph = None;
            world.manifest_error = Some(e.to_string());
        }
    }
}

#[when("its contents are checked")]
fn graph_checked(world: &mut CliWorld) {
    assert_graph(world);
}

#[when("the graph contents are checked")]
fn graph_contents_checked(world: &mut CliWorld) {
    assert_graph(world);
}

#[when("the generation result is checked")]
fn generation_result_checked(world: &mut CliWorld) {
    assert_generation_attempted(world);
}

#[then("IR generation fails")]
fn ir_generation_fails(world: &mut CliWorld) {
    assert!(
        world.manifest_error.is_some(),
        "expected IR generation error",
    );
}

#[when("an action is removed from the graph")]
fn remove_action(world: &mut CliWorld) {
    let graph = world.build_graph.as_mut().expect("graph");
    let first_action = graph.targets.values().next().map(|e| e.action_id.clone());
    if let Some(id) = first_action {
        graph.actions.remove(&id);
        world.removed_action_id = Some(id);
    }
}
