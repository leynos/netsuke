//! Proptest strategy for generated Ninja build edges.
//!
//! The separator properties in the parent module generate edges with varied
//! input, implicit, and order-only dependency counts. Generating them here
//! keeps the parent module inside the repository's module-size limit.

use proptest::prelude::*;
use test_support::ninja_gen::paths_strategy;

use crate::ir::BuildEdge;

/// Build a strategy for edges drawing dependency paths from the given ranges.
pub(super) fn edge_strategy_with_ranges(
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
                    dependency_order: crate::ir::DependencyOrder::Parallel,
                    explicit_outputs,
                    implicit_outputs,
                    order_only_deps,
                    progress_label: None,
                    phony: false,
                    always: false,
                    console: false,
                }
            },
        )
}
