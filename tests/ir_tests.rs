//! Unit tests for IR structures.
//!
//! Directly constructs [`netsuke::ir::BuildGraph`], [`BuildEdge`], and
//! [`Action`] values and asserts their default state, field semantics, and
//! canonical edge ownership and output-index lookup. Does not exercise manifest
//! parsing or Ninja generation.

use camino::Utf8PathBuf;
use netsuke::ast::Recipe;
use netsuke::ir::{Action, BuildEdge, BuildGraph};
use rstest::rstest;

#[rstest]
fn build_graph_default_is_empty() {
    let graph = BuildGraph::default();
    assert!(graph.actions.is_empty());
    assert_eq!(graph.edges, Vec::new());
    assert!(graph.targets.is_empty());
    assert_eq!(graph.default_targets, Vec::<Utf8PathBuf>::new());
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
        inputs: vec![Utf8PathBuf::from("in")],
        implicit_deps: Vec::new(),
        dependency_order: netsuke::ir::DependencyOrder::Parallel,
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: true,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("id".into(), action);
    graph.insert_edge(edge);
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
    let first_insert = graph.actions.insert("a".into(), action1);
    assert!(first_insert.is_none());
    let second_insert = graph.actions.insert("a".into(), action2);
    assert!(second_insert.is_some());
    assert_eq!(graph.actions.len(), 1);
    let Some(action) = graph.actions.get("a") else {
        panic!("expected action for id 'a'");
    };
    if let Recipe::Command { command } = &action.recipe {
        assert_eq!(command.as_single(), Some("two"));
    } else {
        panic!("unexpected recipe type");
    }
}

#[test]
fn build_graph_indexes_canonical_targets() {
    let mut graph = BuildGraph::default();
    let edge1 = BuildEdge {
        action_id: "a".into(),
        inputs: vec![Utf8PathBuf::from("in")],
        implicit_deps: Vec::new(),
        dependency_order: netsuke::ir::DependencyOrder::Parallel,
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    };
    let edge_id = graph.insert_edge(edge1);
    assert_eq!(graph.targets.len(), 1);
    assert_eq!(graph.edges.len(), 1);
    let out_key = Utf8PathBuf::from("out");
    let Some((stored_output, edge)) = graph.target_for_output(out_key.as_path()) else {
        panic!("expected edge for out");
    };
    assert_eq!(stored_output, out_key);
    assert_eq!(graph.targets.get(&out_key), Some(&edge_id));
    assert!(!edge.always);
}
