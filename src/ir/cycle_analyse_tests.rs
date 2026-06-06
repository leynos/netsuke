//! Public-boundary tests for IR cycle analysis reports.
//!
//! Exercises `analyse` across cycle and missing-dependency combinations so
//! the report contract is checked separately from detector internals.

use super::*;
use std::collections::HashMap;

/// Add one missing implicit dependency to the first node in an acyclic graph.
fn make_acyclic_chain_with_missing_dependency(
    nodes: &[camino::Utf8PathBuf],
) -> HashMap<camino::Utf8PathBuf, super::super::super::BuildEdge> {
    let mut targets = make_acyclic_chain(nodes);
    let Some(first) = nodes.first() else {
        return targets;
    };
    let mut builder = EdgeBuilder::new(first.clone()).implicit_dep(path("missing"));
    if let Some(next) = nodes.get(1) {
        builder = builder.input(next.clone());
    }
    targets.insert(first.clone(), builder.build());
    targets
}

/// `analyse` reports missing dependencies discovered before the first cycle.
#[test]
fn analyse_reports_missing_dependencies_before_detected_cycle() {
    let mut targets = HashMap::new();
    targets.insert(
        path("a"),
        EdgeBuilder::new(path("a"))
            .input(path("missing"))
            .implicit_dep(path("also_missing"))
            .build(),
    );
    targets.insert(
        path("b"),
        EdgeBuilder::new(path("b")).input(path("c")).build(),
    );
    targets.insert(
        path("c"),
        EdgeBuilder::new(path("c")).input(path("b")).build(),
    );

    let report = analyse(&targets);

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
    let mut targets = HashMap::new();
    targets.insert(
        path("a"),
        EdgeBuilder::new(path("a")).input(path("b")).build(),
    );
    targets.insert(path("b"), EdgeBuilder::new(path("b")).build());

    let report = analyse(&targets);

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
    let mut targets = HashMap::new();
    targets.insert(
        path("a"),
        EdgeBuilder::new(path("a"))
            .input(path("b"))
            .implicit_dep(path("missing"))
            .build(),
    );
    targets.insert(path("b"), EdgeBuilder::new(path("b")).build());

    let report = analyse(&targets);

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

/// `analyse` reports cycles and no missing dependencies for complete cycles.
#[test]
fn analyse_returns_cycle_with_empty_missing_dependencies() {
    let mut targets = HashMap::new();
    targets.insert(
        path("a"),
        EdgeBuilder::new(path("a")).input(path("b")).build(),
    );
    targets.insert(
        path("b"),
        EdgeBuilder::new(path("b")).input(path("a")).build(),
    );

    let report = analyse(&targets);

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
        let targets = make_acyclic_chain(&nodes);
        let report = analyse(&targets);

        prop_assert!(report.cycle.is_none());
        prop_assert!(report.missing_dependencies.is_empty());
    }

    /// analyse() reports the expected cycle and no missing dependencies.
    #[test]
    fn analyse_cycle_graphs_report_cycle_without_missing_dependencies(count in 2usize..=8) {
        let nodes = sequential_nodes(count);
        let targets = make_cycle_graph(&nodes);
        let expected = canonicalize_cycle(make_cycle(&nodes));
        let report = analyse(&targets);

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
