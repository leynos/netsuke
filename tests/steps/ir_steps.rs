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

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[when(expr = "the manifest file {string} is compiled to IR")]
fn compile_manifest(world: &mut CliWorld, path: String) {
    match netsuke::manifest::from_path(&path)
        .and_then(|m| BuildGraph::from_manifest(&m).map_err(anyhow::Error::from))
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

#[then("IR generation fails")]
fn ir_generation_fails(world: &mut CliWorld) {
    assert!(
        world.manifest_error.is_some(),
        "expected IR generation error"
    );
}
