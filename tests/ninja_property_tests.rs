//! Property-based tests for Ninja build-line separator ordering.
//!
//! PR #315 established the separator contract for generated build lines:
//! explicit inputs, then `|` implicit dependencies, then `||` order-only
//! dependencies. These tests pin that ordering across generated dependency
//! lists.

use netsuke::ast::Recipe;
use netsuke::ir::{Action, BuildEdge, BuildGraph};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use std::collections::HashMap;

fn paths(values: &[String]) -> Vec<camino::Utf8PathBuf> {
    values.iter().map(camino::Utf8PathBuf::from).collect()
}

fn graph_with_edge(
    inputs: &[String],
    implicit_deps: &[String],
    order_only_deps: &[String],
) -> BuildGraph {
    let action = Action {
        recipe: Recipe::Command {
            command: "touch out".into(),
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let edge = BuildEdge {
        action_id: "act".into(),
        inputs: paths(inputs),
        implicit_deps: paths(implicit_deps),
        explicit_outputs: vec!["out".into()],
        implicit_outputs: Vec::new(),
        order_only_deps: paths(order_only_deps),
        phony: false,
        always: false,
    };
    let mut actions = HashMap::new();
    actions.insert("act".to_owned(), action);
    let mut targets = HashMap::new();
    targets.insert(camino::Utf8PathBuf::from("out"), edge);
    BuildGraph {
        actions,
        targets,
        default_targets: Vec::new(),
    }
}

fn build_line(
    inputs: &[String],
    implicit_deps: &[String],
    order_only_deps: &[String],
) -> Result<String, TestCaseError> {
    let graph = graph_with_edge(inputs, implicit_deps, order_only_deps);
    let ninja = netsuke::ninja_gen::generate(&graph)
        .map_err(|e| TestCaseError::fail(format!("generate failed: {e}")))?;
    ninja
        .lines()
        .find(|line| line.starts_with("build "))
        .map(str::to_owned)
        .ok_or_else(|| TestCaseError::fail(format!("no build line in:\n{ninja}")))
}

fn name_list(max: usize) -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec("[a-z][a-z0-9]{0,7}", 1..=max)
}

proptest! {
    /// With all three dependency classes present, the emitted line always
    /// places `|` after explicit inputs and before `||`.
    #[test]
    fn separators_follow_inputs_then_implicit_then_order_only(
        inputs in name_list(4),
        implicit in name_list(4),
        order_only in name_list(4),
    ) {
        let line = build_line(&inputs, &implicit, &order_only)?;
        let expected = format!(
            "build out: act {} | {} || {}",
            inputs.join(" "),
            implicit.join(" "),
            order_only.join(" "),
        );
        prop_assert_eq!(line, expected);
    }

    /// The `|` separator is absent when `implicit_deps` is empty, while the
    /// order-only `||` separator is still emitted.
    #[test]
    fn implicit_separator_absent_when_no_implicit_deps(
        inputs in name_list(4),
        order_only in name_list(4),
    ) {
        let line = build_line(&inputs, &[], &order_only)?;
        let expected = format!(
            "build out: act {} || {}",
            inputs.join(" "),
            order_only.join(" "),
        );
        prop_assert_eq!(&line, &expected);
        prop_assert!(!line.contains(" | "), "unexpected implicit separator: {}", line);
    }
}
