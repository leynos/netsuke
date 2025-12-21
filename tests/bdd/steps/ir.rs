//! Step definitions for BuildGraph (IR) scenarios.

use crate::bdd::fixtures::{RefCellOptionExt, strip_quotes, with_world};
use anyhow::{Context, Result, anyhow, ensure};
use netsuke::ir::BuildGraph;
use rstest_bdd_macros::{given, then, when};

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Check that IR generation was attempted (either graph or error present).
fn assert_generation_attempted() -> Result<()> {
    with_world(|world| {
        match (
            world.build_graph.is_some(),
            world.generation_error.is_filled(),
        ) {
            (true, false) | (false, true) => Ok(()),
            (true, true) => Err(anyhow!("unexpected: graph and error both present")),
            (false, false) => Err(anyhow!("IR generation not attempted")),
        }
    })
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("a new BuildGraph is created")]
fn create_graph() -> Result<()> {
    with_world(|world| {
        world.build_graph.set_value(BuildGraph::default());
    });
    Ok(())
}

#[given("the manifest file {path} is compiled to IR")]
fn compile_manifest_given(path: String) -> Result<()> {
    compile_manifest_impl(strip_quotes(&path))
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("a new BuildGraph is created")]
fn when_create_graph() -> Result<()> {
    with_world(|world| {
        world.build_graph.set_value(BuildGraph::default());
    });
    Ok(())
}

#[when("its contents are checked")]
fn graph_checked() -> Result<()> {
    with_world(|world| {
        ensure!(
            world.build_graph.is_some(),
            "build graph should be available"
        );
        Ok(())
    })
}

#[when("the graph contents are checked")]
fn graph_contents_checked() -> Result<()> {
    with_world(|world| {
        ensure!(
            world.build_graph.is_some(),
            "build graph should be available"
        );
        Ok(())
    })
}

#[when("the generation result is checked")]
fn generation_result_checked() -> Result<()> {
    assert_generation_attempted()
}

#[when("the manifest file {path} is compiled to IR")]
fn compile_manifest_when(path: String) -> Result<()> {
    compile_manifest_impl(strip_quotes(&path))
}

#[when("an action is removed from the graph")]
fn remove_action() -> Result<()> {
    with_world(|world| {
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
    })
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("the graph has {count:usize} actions")]
fn graph_actions(count: usize) -> Result<()> {
    with_world(|world| {
        let actions_len = world
            .build_graph
            .with_ref(|g| g.actions.len())
            .context("build graph should be available")?;
        ensure!(
            actions_len == count,
            "expected {count} actions, found {actions_len}"
        );
        Ok(())
    })
}

#[then("the graph has {count:usize} targets")]
fn graph_targets(count: usize) -> Result<()> {
    with_world(|world| {
        let targets_len = world
            .build_graph
            .with_ref(|g| g.targets.len())
            .context("build graph should be available")?;
        ensure!(
            targets_len == count,
            "expected {count} targets, found {targets_len}"
        );
        Ok(())
    })
}

#[then("the graph has {count:usize} default targets")]
fn graph_defaults(count: usize) -> Result<()> {
    with_world(|world| {
        let defaults_len = world
            .build_graph
            .with_ref(|g| g.default_targets.len())
            .context("build graph should be available")?;
        ensure!(
            defaults_len == count,
            "expected {count} default targets, found {defaults_len}"
        );
        Ok(())
    })
}

#[then("IR generation fails")]
fn ir_generation_fails() -> Result<()> {
    with_world(|world| {
        ensure!(
            world.generation_error.is_filled(),
            "expected IR generation error, but none present"
        );
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Implementation helpers
// ---------------------------------------------------------------------------

/// Compile a manifest file to IR, storing result or error in state.
fn compile_manifest_impl(path: &str) -> Result<()> {
    with_world(|world| {
        match netsuke::manifest::from_path(path)
            .and_then(|m| BuildGraph::from_manifest(&m).context("building IR from manifest"))
            .with_context(|| format!("IR generation failed for {path}"))
        {
            Ok(graph) => {
                world.build_graph.set_value(graph);
                world.generation_error.clear();
            }
            Err(e) => {
                world.build_graph.clear_value();
                world.generation_error.set(e.to_string());
            }
        }
    });
    Ok(())
}
