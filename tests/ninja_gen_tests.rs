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
