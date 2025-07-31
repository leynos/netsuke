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
fn generate_script_rule_with_fields() {
    let action = Action {
        recipe: Recipe::Script {
            script: "echo hi\necho there".into(),
        },
        description: Some("desc".into()),
        depfile: Some("file.d".into()),
        deps_format: Some("gcc".into()),
        pool: Some("pool".into()),
        restat: true,
    };
    let edge = BuildEdge {
        action_id: "a".into(),
        inputs: vec![PathBuf::from("in")],
        explicit_outputs: vec![PathBuf::from("out")],
        implicit_outputs: vec![PathBuf::from("imp")],
        order_only_deps: vec![PathBuf::from("oo")],
        phony: false,
        always: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("a".into(), action);
    graph.targets.insert(PathBuf::from("out"), edge);
    graph.default_targets.push(PathBuf::from("out"));

    let ninja = generate(&graph);
    let expected = concat!(
        "rule a\n",
        "  command = /bin/sh -e -c \"\n",
        "    echo hi\n",
        "    echo there\n",
        "  \"\n",
        "  description = desc\n",
        "  depfile = file.d\n",
        "  deps = gcc\n",
        "  pool = pool\n",
        "  restat = 1\n\n",
        "build out | imp: a in || oo\n\n",
        "default out\n",
    );
    assert_eq!(ninja, expected);
}

#[rstest(
    action_restat,
    always,
    case(false, true),
    case(true, true),
    case(false, false)
)]
fn restat_for_always_edges(action_restat: bool, always: bool) {
    let action = Action {
        recipe: Recipe::Command {
            command: "true".into(),
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: action_restat,
    };
    let edge = BuildEdge {
        action_id: "a".into(),
        inputs: vec![PathBuf::from("in")],
        explicit_outputs: vec![PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("a".into(), action);
    graph.targets.insert(PathBuf::from("out"), edge);

    let ninja = generate(&graph);
    let build_restat = concat!("build out: a in\n", "  restat = 1\n");
    let has_build_restat = ninja.contains(build_restat);
    assert_eq!(has_build_restat, !action_restat && always);
}
