//! Characterize cycle detection through output aliases.

use camino::Utf8PathBuf;
use googletest::prelude::*;
use pretty_assertions::assert_eq;
use rstest::rstest;

use super::super::graph::{BuildEdge, BuildGraph};
use super::analyse;

/// Describe one edge and the cycle expected from a graph containing it.
#[derive(Clone, Copy)]
struct CycleCase<'a> {
    edges: &'a [EdgeSpec<'a>],
    expected_cycle: &'a [&'a str],
}

/// Specify output aliases and dependency classes for one test edge.
#[derive(Clone, Copy)]
struct EdgeSpec<'a> {
    explicit_outputs: &'a [&'a str],
    implicit_outputs: &'a [&'a str],
    inputs: &'a [&'a str],
    implicit_deps: &'a [&'a str],
    order_only_deps: &'a [&'a str],
}

/// Build an edge from named test paths.
fn build_edge(spec: EdgeSpec<'_>) -> BuildEdge {
    BuildEdge {
        action_id: "test".into(),
        progress_label: None,
        inputs: paths(spec.inputs),
        implicit_deps: paths(spec.implicit_deps),
        dependency_order: crate::ir::DependencyOrder::Parallel,
        explicit_outputs: paths(spec.explicit_outputs),
        implicit_outputs: paths(spec.implicit_outputs),
        order_only_deps: paths(spec.order_only_deps),
        phony: false,
        always: false,
    }
}

/// Convert named paths into the graph's UTF-8 path representation.
fn paths(names: &[&str]) -> Vec<Utf8PathBuf> {
    names.iter().map(|name| Utf8PathBuf::from(*name)).collect()
}

/// Build the specified graph and compare its canonical cycle path.
fn assert_cycle(case: CycleCase<'_>) {
    let mut graph = BuildGraph::default();
    for edge in case.edges {
        graph
            .insert_edge(build_edge(*edge))
            .expect("test edges use unique output aliases");
    }

    let report = analyse(&graph);
    let expected = paths(case.expected_cycle);
    assert_that!(report.cycle.as_ref(), some(eq(&expected)));
    assert_eq!(report.cycle, Some(expected));
    assert_eq!(report.missing_dependencies, Vec::new());
}

#[rstest]
#[case::implicit_output_closes_cycle(CycleCase {
    edges: &[
        EdgeSpec {
            explicit_outputs: &["a"],
            implicit_outputs: &["a.extra"],
            inputs: &["b"],
            implicit_deps: &[],
            order_only_deps: &[],
        },
        EdgeSpec {
            explicit_outputs: &["b"],
            implicit_outputs: &[],
            inputs: &["a.extra"],
            implicit_deps: &[],
            order_only_deps: &[],
        },
    ],
    expected_cycle: &["a.extra", "b", "a.extra"],
})]
#[case::implicit_output_self_cycle(CycleCase {
    edges: &[EdgeSpec {
        explicit_outputs: &["a"],
        implicit_outputs: &["x"],
        inputs: &["x"],
        implicit_deps: &[],
        order_only_deps: &[],
    }],
    expected_cycle: &["x", "x"],
})]
#[case::non_first_explicit_output_closes_cycle(CycleCase {
    edges: &[
        EdgeSpec {
            explicit_outputs: &["a", "a.extra"],
            implicit_outputs: &[],
            inputs: &["b"],
            implicit_deps: &[],
            order_only_deps: &[],
        },
        EdgeSpec {
            explicit_outputs: &["b"],
            implicit_outputs: &[],
            inputs: &["a.extra"],
            implicit_deps: &[],
            order_only_deps: &[],
        },
    ],
    expected_cycle: &["a.extra", "b", "a.extra"],
})]
fn implicit_outputs_participate(#[case] case: CycleCase<'_>) {
    assert_cycle(case);
}

#[rstest]
#[case::order_only_self_edge(CycleCase {
    edges: &[EdgeSpec {
        explicit_outputs: &["a"],
        implicit_outputs: &[],
        inputs: &[],
        implicit_deps: &[],
        order_only_deps: &["a"],
    }],
    expected_cycle: &["a", "a"],
})]
#[case::order_only_two_node(CycleCase {
    edges: &[
        EdgeSpec {
            explicit_outputs: &["tool"],
            implicit_outputs: &[],
            inputs: &[],
            implicit_deps: &[],
            order_only_deps: &["headers.stamp"],
        },
        EdgeSpec {
            explicit_outputs: &["headers.stamp"],
            implicit_outputs: &[],
            inputs: &["tool"],
            implicit_deps: &[],
            order_only_deps: &[],
        },
    ],
    expected_cycle: &["headers.stamp", "tool", "headers.stamp"],
})]
#[case::order_only_three_node(CycleCase {
    edges: &[
        EdgeSpec {
            explicit_outputs: &["a"],
            implicit_outputs: &[],
            inputs: &[],
            implicit_deps: &[],
            order_only_deps: &["b"],
        },
        EdgeSpec {
            explicit_outputs: &["b"],
            implicit_outputs: &[],
            inputs: &[],
            implicit_deps: &[],
            order_only_deps: &["c"],
        },
        EdgeSpec {
            explicit_outputs: &["c"],
            implicit_outputs: &[],
            inputs: &[],
            implicit_deps: &[],
            order_only_deps: &["a"],
        },
    ],
    expected_cycle: &["a", "b", "c", "a"],
})]
#[case::order_only_closes_mixed_cycle(CycleCase {
    edges: &[
        EdgeSpec {
            explicit_outputs: &["a"],
            implicit_outputs: &[],
            inputs: &["b"],
            implicit_deps: &[],
            order_only_deps: &[],
        },
        EdgeSpec {
            explicit_outputs: &["b"],
            implicit_outputs: &[],
            inputs: &[],
            implicit_deps: &["c"],
            order_only_deps: &[],
        },
        EdgeSpec {
            explicit_outputs: &["c"],
            implicit_outputs: &[],
            inputs: &[],
            implicit_deps: &[],
            order_only_deps: &["a"],
        },
    ],
    expected_cycle: &["a", "b", "c", "a"],
})]
fn order_only_dependencies_participate(#[case] case: CycleCase<'_>) {
    assert_cycle(case);
}
