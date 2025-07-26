//! Unit tests for IR structures.

use netsuke::ast::Recipe;
use netsuke::ir::{Action, BuildEdge, BuildGraph};
use rstest::rstest;
use std::path::PathBuf;

#[rstest]
fn build_graph_default_is_empty() {
    let graph = BuildGraph::default();
    assert!(graph.actions.is_empty());
    assert!(graph.targets.is_empty());
    assert!(graph.default_targets.is_empty());
}

#[rstest]
fn create_action_and_edge() {
    let action = Action {
        recipe: Recipe::Command {
            command: "echo".into(),
        },
        description: Some("desc".into()),
        depfile: Some("$out.d".into()),
        deps_format: Some("gcc".into()),
        pool: None,
        restat: false,
    };
    let edge = BuildEdge {
        action_id: "id".into(),
        inputs: vec![PathBuf::from("in")],
        explicit_outputs: vec![PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: true,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("id".into(), action);
    graph.targets.insert(PathBuf::from("out"), edge);
    assert_eq!(graph.actions.len(), 1);
    assert_eq!(graph.targets.len(), 1);
}
