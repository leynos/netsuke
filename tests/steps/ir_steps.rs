//! Step definitions for `BuildGraph` scenarios.

use crate::CliWorld;
use cucumber::{then, when};
use netsuke::ir::BuildGraph;

#[when("a new BuildGraph is created")]
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
