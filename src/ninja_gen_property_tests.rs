//! Property tests for Ninja build edge separator formatting.
//!
//! `implicit_deps_separator_precedes_order_only_separator` verifies the
//! dependency-side ` | ` appears after explicit inputs and before ` || `.
//! `implicit_deps_separator_is_absent_when_empty` verifies the dependency-side
//! implicit separator is omitted when `implicit_deps` is empty.

use proptest::prelude::*;
use test_support::ninja_gen::paths_strategy;

use super::DisplayEdge;
use crate::ir::BuildEdge;

fn edge_strategy_with_ranges(
    input_range: std::ops::Range<usize>,
    implicit_range: std::ops::Range<usize>,
    order_only_range: std::ops::Range<usize>,
) -> impl Strategy<Value = BuildEdge> {
    (
        "[a-z][a-z0-9_]{0,8}",
        paths_strategy("in", input_range),
        paths_strategy("out", 1..5),
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

fn build_line(formatted: &str) -> Option<&str> {
    formatted.lines().next()
}

fn dependency_side(line: &str) -> Option<&str> {
    line.split_once(": ").map(|(_, deps)| deps)
}

fn bare_pipe_position(line: &str) -> Option<usize> {
    line.match_indices(" | ").map(|(index, _)| index).next()
}

proptest! {
    #[test]
    fn implicit_deps_separator_precedes_order_only_separator(edge in edge_strategy_with_ranges(1..5, 1..5, 1..5)) {
        let formatted = format_edge(&edge);
        let line = build_line(&formatted).expect("build line should be emitted");
        let deps = dependency_side(line).expect("build line should contain rule separator");
        let implicit_pos = bare_pipe_position(deps).expect("implicit separator should be emitted");
        let order_pos = deps.find(" || ").expect("order-only separator should be emitted");
        let (_, dependency_groups) = deps
            .split_once(' ')
            .expect("action identifier should precede dependencies");
        let (before_order_only, order_only) = dependency_groups
            .split_once(" || ")
            .expect("order-only separator should be emitted");
        let (inputs, implicit) = before_order_only
            .split_once(" | ")
            .expect("implicit separator should be emitted");

        prop_assert!(implicit_pos < order_pos);
        prop_assert_eq!(inputs.split_whitespace().collect::<Vec<_>>(), edge.inputs.iter().map(|path| path.as_str()).collect::<Vec<_>>());
        prop_assert_eq!(implicit.split_whitespace().collect::<Vec<_>>(), edge.implicit_deps.iter().map(|path| path.as_str()).collect::<Vec<_>>());
        prop_assert_eq!(order_only.split_whitespace().collect::<Vec<_>>(), edge.order_only_deps.iter().map(|path| path.as_str()).collect::<Vec<_>>());
    }

    #[test]
    fn implicit_deps_separator_is_absent_when_empty(edge in edge_strategy_with_ranges(0..5, 0..1, 1..5)) {
        let formatted = format_edge(&edge);
        let line = build_line(&formatted).expect("build line should be emitted");
        let deps = dependency_side(line).expect("build line should contain rule separator");

        prop_assert!(bare_pipe_position(deps).is_none());
        prop_assert!(deps.contains(" || "));
    }
}
