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

#[test]
fn build_graph_duplicate_action_ids() {
    let mut graph = BuildGraph::default();
    let action1 = Action {
        recipe: Recipe::Command {
            command: "one".into(),
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let action2 = Action {
        recipe: Recipe::Command {
            command: "two".into(),
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let prev = graph.actions.insert("a".into(), action1);
    assert!(prev.is_none());
    let prev = graph.actions.insert("a".into(), action2);
    assert!(prev.is_some());
    assert_eq!(graph.actions.len(), 1);
    if let Recipe::Command { command } = &graph.actions.get("a").expect("action").recipe {
        assert_eq!(command, "two");
    } else {
        panic!("unexpected recipe type");
    }
}

#[test]
fn build_graph_duplicate_targets() {
    let mut graph = BuildGraph::default();
    let edge1 = BuildEdge {
        action_id: "a".into(),
        inputs: vec![PathBuf::from("in")],
        explicit_outputs: vec![PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    };
    let edge2 = BuildEdge {
        action_id: "a".into(),
        inputs: vec![PathBuf::from("in")],
        explicit_outputs: vec![PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: true,
    };
    let prev = graph.targets.insert(PathBuf::from("out"), edge1);
    assert!(prev.is_none());
    let prev = graph.targets.insert(PathBuf::from("out"), edge2);
    assert!(prev.is_some());
    assert_eq!(graph.targets.len(), 1);
    assert!(
        graph
            .targets
            .get(&PathBuf::from("out"))
            .expect("edge")
            .always
    );
}
