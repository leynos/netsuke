//! Property tests for Ninja build edge separator formatting.
//!
//! `implicit_deps_separator_precedes_order_only_separator` verifies the
//! dependency-side ` | ` appears after explicit inputs and before ` || `.
//! `implicit_deps_separator_is_absent_when_empty` verifies the dependency-side
//! implicit separator is omitted when `implicit_deps` is empty.

use camino::Utf8PathBuf;
use proptest::prelude::*;
use test_support::ninja_gen::paths_strategy;

use super::DisplayEdge;
use crate::ir::BuildEdge;

fn path_strategy(prefix: &'static str) -> impl Strategy<Value = Utf8PathBuf> {
    (0usize..100).prop_map(move |index| Utf8PathBuf::from(format!("{prefix}{index}")))
}

fn edge_strategy() -> impl Strategy<Value = BuildEdge> {
    edge_strategy_with_ranges(0..5, 0..5, 0..5)
}

fn edge_strategy_with_ranges(
    input_range: std::ops::Range<usize>,
    implicit_range: std::ops::Range<usize>,
    order_only_range: std::ops::Range<usize>,
) -> impl Strategy<Value = BuildEdge> {
    (
        "[a-z][a-z0-9_]{0,8}",
        paths_strategy("in", input_range),
        prop::collection::vec(path_strategy("out"), 1..5),
        paths_strategy("iout", 0..5),
        paths_strategy("imp", implicit_range),
        paths_strategy("order", order_only_range),
    )
        .prop_map(
            |(
                action_id,
                inputs,
                explicit_outputs,
                implicit_outputs,
                implicit_deps,
                order_only_deps,
            )| {
                BuildEdge {
                    action_id,
                    inputs,
                    implicit_deps,
                    explicit_outputs,
                    implicit_outputs,
                    order_only_deps,
                    phony: false,
                    always: false,
                }
            },
        )
}

fn format_edge(edge: &BuildEdge) -> String {
    DisplayEdge {
        edge,
        action_restat: false,
    }
    .to_string()
}

fn build_line(formatted: &str) -> &str {
    formatted.lines().next().expect("build line")
}

fn dependency_side(line: &str) -> &str {
    line.split_once(": ")
        .map(|(_, deps)| deps)
        .expect("build line should contain rule separator")
}

fn bare_pipe_position(line: &str) -> Option<usize> {
    line.match_indices(" | ").map(|(index, _)| index).next()
}

proptest! {
    #[test]
    fn implicit_deps_separator_precedes_order_only_separator(edge in edge_strategy_with_ranges(1..5, 1..5, 1..5)) {
        let formatted = format_edge(&edge);
        let line = build_line(&formatted);
        let deps = dependency_side(line);
        let implicit_pos = bare_pipe_position(deps).expect("implicit separator should be emitted");
        let order_pos = deps.find(" || ").expect("order-only separator should be emitted");
        let (before_implicit, _) = deps.split_at(implicit_pos);
        let first_input = edge.inputs.first().expect("input should exist");

        prop_assert!(before_implicit.contains(first_input.as_str()));
        prop_assert!(implicit_pos < order_pos);
    }

    #[test]
    fn implicit_deps_separator_is_absent_when_empty(mut edge in edge_strategy()) {
        edge.implicit_deps.clear();

        let formatted = format_edge(&edge);
        let line = build_line(&formatted);
        let deps = dependency_side(line);

        prop_assert!(bare_pipe_position(deps).is_none());
    }
}
