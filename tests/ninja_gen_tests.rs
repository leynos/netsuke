//! Unit tests for Ninja file generation.

use netsuke::ast::Recipe;
use netsuke::ir::{Action, BuildEdge, BuildGraph};
use netsuke::ninja_gen::generate;
use rstest::rstest;
use std::path::PathBuf;

#[rstest]
fn generate_phony() {
    let action = Action {
        recipe: Recipe::Command {
            command: "true".into(),
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let edge = BuildEdge {
        action_id: "a".into(),
        inputs: vec![PathBuf::from("in")],
        explicit_outputs: vec![PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: true,
        always: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("a".into(), action);
    graph.targets.insert(PathBuf::from("out"), edge);

    let ninja = generate(&graph);
    let expected = concat!(
        "rule a\n",
        "  command = true\n\n",
        "build out: phony in\n\n",
    );
    assert_eq!(ninja, expected);
}

#[rstest]
fn generate_standard_build() {
    let action = Action {
        recipe: Recipe::Command {
            command: "cc -c $in -o $out".into(),
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let edge = BuildEdge {
        action_id: "compile".into(),
        inputs: vec![PathBuf::from("a.c"), PathBuf::from("b.c")],
        explicit_outputs: vec![PathBuf::from("ab.o")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("compile".into(), action);
    graph.targets.insert(PathBuf::from("ab.o"), edge);

    let ninja = generate(&graph);
    let expected = concat!(
        "rule compile\n",
        "  command = cc -c $in -o $out\n\n",
        "build ab.o: compile a.c b.c\n\n",
    );
    assert_eq!(ninja, expected);
}

#[rstest]
fn generate_complex_dependencies() {
    let action = Action {
        recipe: Recipe::Command {
            command: "true".into(),
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let edge = BuildEdge {
        action_id: "b".into(),
        inputs: vec![PathBuf::from("in")],
        explicit_outputs: vec![PathBuf::from("out"), PathBuf::from("log")],
        implicit_outputs: vec![PathBuf::from("out.d")],
        order_only_deps: vec![PathBuf::from("stamp")],
        phony: false,
        always: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("b".into(), action);
    graph.targets.insert(PathBuf::from("out"), edge);

    let ninja = generate(&graph);
    let expected = concat!(
        "rule b\n",
        "  command = true\n\n",
        "build out log | out.d: b in || stamp\n\n",
    );
    assert_eq!(ninja, expected);
}

#[rstest]
fn generate_empty_graph() {
    let graph = BuildGraph::default();
    let ninja = generate(&graph);
    assert!(ninja.is_empty());
}
