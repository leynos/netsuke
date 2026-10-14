//! Public-boundary tests for IR cycle analysis reports.
//!
//! Exercises `analyse` across cycle and missing-dependency combinations so
//! the report contract is checked separately from detector internals.

use proptest::prelude::*;

use super::super::super::graph::{BuildEdge, BuildGraph};
use super::super::support::canonicalize_cycle;
use super::{
    EdgeBuilder, analyse, make_acyclic_chain, make_cycle, make_cycle_graph, path, sequential_nodes,
};

/// Add one missing implicit dependency to the first node in an acyclic graph.
fn make_acyclic_chain_with_missing_dependency(nodes: &[camino::Utf8PathBuf]) -> BuildGraph {
    let mut graph = make_acyclic_chain(nodes);
    let Some(first) = nodes.first() else {
        return graph;
    };
    let mut builder = EdgeBuilder::new(first.clone()).implicit_dep(path("missing"));
    if let Some(next) = nodes.get(1) {
        builder = builder.input(next.clone());
    }
    let _ = graph.replace_edge_for_output(first.as_path(), builder.build());
    graph
}

/// `analyse` reports missing dependencies discovered before the first cycle.
#[test]
fn analyse_reports_missing_dependencies_before_detected_cycle() {
    let mut graph = BuildGraph::default();
    graph
        .insert_edge(
            EdgeBuilder::new(path("a"))
                .input(path("missing"))
                .implicit_dep(path("also_missing"))
                .build(),
        )
        .expect("test graph output aliases must be unique");
    graph
        .insert_edge(EdgeBuilder::new(path("b")).input(path("c")).build())
        .expect("test graph output aliases must be unique");
    graph
        .insert_edge(EdgeBuilder::new(path("c")).input(path("b")).build())
        .expect("test graph output aliases must be unique");

    let report = analyse(&graph);

    assert_eq!(report.cycle, Some(vec![path("b"), path("c"), path("b")]));
    assert_eq!(
        report.missing_dependencies,
        vec![
            (path("a"), path("missing")),
            (path("a"), path("also_missing")),
        ],
    );
}

/// `analyse` reports neither cycles nor missing dependencies for complete DAGs.
#[test]
fn analyse_returns_no_cycle_for_acyclic_graph() {
    let mut graph = BuildGraph::default();
    graph
        .insert_edge(EdgeBuilder::new(path("a")).input(path("b")).build())
        .expect("test graph output aliases must be unique");
    graph
        .insert_edge(EdgeBuilder::new(path("b")).build())
        .expect("test graph output aliases must be unique");

    let report = analyse(&graph);

    assert!(
        report.cycle.is_none(),
        "acyclic graph must produce no cycle"
    );
    assert!(
        report.missing_dependencies.is_empty(),
        "acyclic graph with no missing dependencies must report none",
    );
}

/// `analyse` reports missing dependencies for acyclic graphs.
#[test]
fn analyse_returns_missing_dependencies_for_acyclic_graph() {
    let mut graph = BuildGraph::default();
    graph
        .insert_edge(
            EdgeBuilder::new(path("a"))
                .input(path("b"))
                .implicit_dep(path("missing"))
                .build(),
        )
        .expect("test graph output aliases must be unique");
    graph
        .insert_edge(EdgeBuilder::new(path("b")).build())
        .expect("test graph output aliases must be unique");

    let report = analyse(&graph);

    assert!(
        report.cycle.is_none(),
        "acyclic graph must produce no cycle"
    );
    assert_eq!(
        report.missing_dependencies,
        vec![(path("a"), path("missing"))],
        "acyclic graph must report unresolved dependencies",
    );
}

/// `analyse` resolves a dependency that names a non-first output alias.
///
/// The index owns every alias of a multi-output edge. A lookup that only saw
/// the first alias would report `second` as an unresolved dependency instead
/// of resolving it to the edge that produces it.
#[test]
fn analyse_resolves_non_first_output_alias_as_dependency() {
    let mut graph = BuildGraph::default();
    let producer = BuildEdge {
        action_id: "id".into(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        dependency_order: crate::ir::DependencyOrder::Parallel,
        explicit_outputs: vec![path("first"), path("second")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    };
    graph
        .insert_edge(producer)
        .expect("test graph output aliases must be unique");
    graph
        .insert_edge(
            EdgeBuilder::new(path("consumer"))
                .input(path("second"))
                .build(),
        )
        .expect("test graph output aliases must be unique");

    let report = analyse(&graph);

    assert!(
        report.cycle.is_none(),
        "acyclic graph must produce no cycle"
    );
    assert!(
        report.missing_dependencies.is_empty(),
        "the second alias must resolve to its producing edge, not be missing: {:?}",
        report.missing_dependencies,
    );
}

/// `analyse` reports cycles and no missing dependencies for complete cycles.
#[test]
fn analyse_returns_cycle_with_empty_missing_dependencies() {
    let mut graph = BuildGraph::default();
    graph
        .insert_edge(EdgeBuilder::new(path("a")).input(path("b")).build())
        .expect("test graph output aliases must be unique");
    graph
        .insert_edge(EdgeBuilder::new(path("b")).input(path("a")).build())
        .expect("test graph output aliases must be unique");

    let report = analyse(&graph);

    assert_eq!(
        report.cycle,
        Some(vec![path("a"), path("b"), path("a")]),
        "cyclic graph must report the detected cycle",
    );
    assert!(
        report.missing_dependencies.is_empty(),
        "no missing dependencies must be reported when all targets are present",
    );
}

proptest! {
    /// analyse() reports no cycle and no missing dependencies for DAGs.
    #[test]
    fn analyse_acyclic_chains_report_no_cycle(count in 2usize..=8) {
        let nodes = sequential_nodes(count);
        let graph = make_acyclic_chain(&nodes);
        let report = analyse(&graph);

        prop_assert!(report.cycle.is_none());
        prop_assert!(report.missing_dependencies.is_empty());
    }

    /// analyse() reports the expected cycle and no missing dependencies.
    #[test]
    fn analyse_cycle_graphs_report_cycle_without_missing_dependencies(count in 2usize..=8) {
        let nodes = sequential_nodes(count);
        let graph = make_cycle_graph(&nodes);
        let expected = canonicalize_cycle(make_cycle(&nodes));
        let report = analyse(&graph);

        prop_assert_eq!(report.cycle, Some(expected));
        prop_assert!(report.missing_dependencies.is_empty());
    }

    /// analyse() reports missing dependencies on acyclic graphs.
    #[test]
    fn analyse_acyclic_chains_report_missing_dependencies(count in 2usize..=8) {
        let nodes = sequential_nodes(count);
        let targets = make_acyclic_chain_with_missing_dependency(&nodes);
        let report = analyse(&targets);
        let Some(first) = nodes.first() else {
            prop_assert!(false, "sequential_nodes generates at least two nodes");
            return Ok(());
        };

        prop_assert!(report.cycle.is_none());
        prop_assert_eq!(
            report.missing_dependencies,
            vec![(first.clone(), path("missing"))],
        );
    }
}
