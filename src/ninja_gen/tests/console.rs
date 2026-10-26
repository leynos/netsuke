//! Tests for edge-level Ninja console pool selection.

use super::command_action;
use super::{NinjaGenError, generate, generate_bundle, generate_posix};
use crate::ir::{BuildEdge, BuildGraph, DependencyOrder};
use anyhow::{Result, ensure};
use camino::Utf8PathBuf;

/// Emit console selection on one edge while reusing the action's rule normally.
#[test]
fn console_pool_is_edge_level_and_keeps_shared_action_rules() -> Result<()> {
    let mut graph = BuildGraph::default();
    graph
        .actions
        .insert("shared".into(), command_action("echo ready".into()));
    for (output, console) in [("console.out", true), ("ordinary.out", false)] {
        graph
            .insert_edge(BuildEdge {
                action_id: "shared".into(),
                inputs: Vec::new(),
                implicit_deps: Vec::new(),
                dependency_order: DependencyOrder::Parallel,
                explicit_outputs: vec![Utf8PathBuf::from(output)],
                implicit_outputs: Vec::new(),
                order_only_deps: Vec::new(),
                phony: false,
                always: false,
                console,
            })
            .expect("test graph output aliases must be unique");
    }

    let ninja = generate_posix(&graph)?;
    let expected = concat!(
        "rule shared\n",
        "  command = echo ready\n\n",
        "build console.out: shared\n",
        "  pool = console\n\n",
        "build ordinary.out: shared\n\n",
    );
    ensure!(
        ninja == expected,
        "expected Ninja manifest:\n{expected}\nactual:\n{ninja}"
    );
    Ok(())
}

/// Clear rule-level console scheduling from an edge without the opt-in.
#[test]
fn ordinary_edge_clears_a_shared_console_action_pool() -> Result<()> {
    let mut graph = BuildGraph::default();
    let mut action = command_action("echo ready".into());
    action.pool = Some("console".into());
    graph.actions.insert("shared".into(), action);
    for (output, console) in [("console.out", true), ("ordinary.out", false)] {
        graph
            .insert_edge(BuildEdge {
                action_id: "shared".into(),
                inputs: Vec::new(),
                implicit_deps: Vec::new(),
                dependency_order: DependencyOrder::Parallel,
                explicit_outputs: vec![Utf8PathBuf::from(output)],
                implicit_outputs: Vec::new(),
                order_only_deps: Vec::new(),
                phony: false,
                always: false,
                console,
            })
            .expect("test graph output aliases must be unique");
    }

    let ninja = generate_posix(&graph)?;
    ensure!(
        ninja.contains("pool = console\n\nbuild console.out: shared\n  pool = console\n"),
        "selected edge must retain console scheduling:\n{ninja}"
    );
    ensure!(
        ninja.contains("build ordinary.out: shared\n  pool =\n\n"),
        "ordinary edge must clear the action rule's console pool:\n{ninja}"
    );
    Ok(())
}

/// Accept the redundant combination of an edge flag and `pool = console` metadata.
#[test]
fn console_pool_accepts_matching_action_pool() -> Result<()> {
    let mut graph = BuildGraph::default();
    let mut action = command_action("echo ready".into());
    action.pool = Some("console".into());
    graph.actions.insert("shared".into(), action);
    graph
        .insert_edge(BuildEdge {
            action_id: "shared".into(),
            inputs: Vec::new(),
            implicit_deps: Vec::new(),
            dependency_order: DependencyOrder::Parallel,
            explicit_outputs: vec![Utf8PathBuf::from("console.out")],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            phony: false,
            always: false,
            console: true,
        })
        .expect("test graph output aliases must be unique");

    let ninja = generate_posix(&graph)?;
    ensure!(
        ninja.contains("  pool = console\n\nbuild console.out: shared\n  pool = console\n\n"),
        "matching action and edge pools must both be accepted:\n{ninja}"
    );
    Ok(())
}

/// Preserve the existing missing-action error when a console edge has no action.
#[test]
fn console_validation_leaves_missing_actions_to_generation() -> Result<()> {
    let mut graph = BuildGraph::default();
    graph
        .insert_edge(BuildEdge {
            action_id: "missing".into(),
            inputs: Vec::new(),
            implicit_deps: Vec::new(),
            dependency_order: DependencyOrder::Parallel,
            explicit_outputs: vec![Utf8PathBuf::from("console.out")],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            phony: false,
            always: false,
            console: true,
        })
        .expect("test graph output aliases must be unique");

    let generate_error = generate(&graph).expect_err("missing action must remain an error");
    ensure!(
        matches!(generate_error, NinjaGenError::MissingAction { ref id, .. } if id == "missing"),
        "unexpected direct generation error: {generate_error:?}"
    );
    let bundle_error = generate_bundle(&graph).expect_err("missing action must remain an error");
    ensure!(
        matches!(bundle_error, NinjaGenError::MissingAction { ref id, .. } if id == "missing"),
        "unexpected bundle generation error: {bundle_error:?}"
    );
    Ok(())
}
