//! Step definitions for `BuildGraph` scenarios.
#![allow(
    clippy::shadow_reuse,
    clippy::shadow_unrelated,
    reason = "Cucumber step macros rebind capture names and steps prefer expect"
)]

use crate::CliWorld;
use anyhow::{Context, Result, anyhow, ensure};
use cucumber::{given, then, when};
use netsuke::ir::BuildGraph;

fn graph(world: &CliWorld) -> Result<&BuildGraph> {
    world
        .build_graph
        .as_ref()
        .context("build graph should have been generated")
}

fn graph_mut(world: &mut CliWorld) -> Result<&mut BuildGraph> {
    world
        .build_graph
        .as_mut()
        .context("build graph should have been generated")
}

fn assert_generation_attempted(world: &CliWorld) -> Result<()> {
    match (world.build_graph.is_some(), world.manifest_error.is_some()) {
        (true, false) | (false, true) => Ok(()),
        (true, true) => Err(anyhow!("unexpected: graph and error present")),
        (false, false) => Err(anyhow!("IR generation not attempted")),
    }
}

#[given("a new BuildGraph is created")]
fn create_graph(world: &mut CliWorld) {
    world.build_graph = Some(BuildGraph::default());
}

#[then(expr = "the graph has {int} actions")]
fn graph_actions(world: &mut CliWorld, count: usize) -> Result<()> {
    let g = graph(world)?;
    ensure!(
        g.actions.len() == count,
        "expected {count} actions, found {}",
        g.actions.len()
    );
    Ok(())
}

#[then(expr = "the graph has {int} targets")]
fn graph_targets(world: &mut CliWorld, count: usize) -> Result<()> {
    let g = graph(world)?;
    ensure!(
        g.targets.len() == count,
        "expected {count} targets, found {}",
        g.targets.len()
    );
    Ok(())
}

#[then(expr = "the graph has {int} default targets")]
fn graph_defaults(world: &mut CliWorld, count: usize) -> Result<()> {
    let g = graph(world)?;
    ensure!(
        g.default_targets.len() == count,
        "expected {count} default targets, found {}",
        g.default_targets.len()
    );
    Ok(())
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
fn graph_checked(world: &mut CliWorld) -> Result<()> {
    let _ = graph(world)?;
    Ok(())
}

#[when("the graph contents are checked")]
fn graph_contents_checked(world: &mut CliWorld) -> Result<()> {
    let _ = graph(world)?;
    Ok(())
}

#[when("the generation result is checked")]
fn generation_result_checked(world: &mut CliWorld) -> Result<()> {
    assert_generation_attempted(world)
}

#[then("IR generation fails")]
fn ir_generation_fails(world: &mut CliWorld) -> Result<()> {
    ensure!(
        world.manifest_error.is_some(),
        "expected IR generation error",
    );
    Ok(())
}

#[when("an action is removed from the graph")]
fn remove_action(world: &mut CliWorld) -> Result<()> {
    let graph = graph_mut(world)?;
    let first_action = graph.targets.values().next().map(|e| e.action_id.clone());
    if let Some(id) = first_action {
        graph.actions.remove(&id);
        world.removed_action_id = Some(id);
        return Ok(());
    }
    Err(anyhow!("no actions available to remove"))
}
