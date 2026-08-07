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

/// Build a graph of `count` nodes with the given forward `(from, to)` edges.
///
/// Forward edges (`to > from`) cannot create back-edges, so the result is a
/// directed acyclic graph by construction.
fn make_forward_edge_graph(
    count: usize,
    edges: &[(usize, usize)],
) -> HashMap<camino::Utf8PathBuf, super::super::super::BuildEdge> {
    let nodes = sequential_nodes(count);
    let mut inputs: Vec<Vec<camino::Utf8PathBuf>> = vec![Vec::new(); count];
    for &(from, to) in edges {
        if to > from
            && let (Some(target), Some(dep)) = (inputs.get_mut(from), nodes.get(to))
        {
            target.push(dep.clone());
        }
    }
    nodes
        .iter()
        .zip(inputs)
        .map(|(name, deps)| {
            let mut builder = EdgeBuilder::new(name.clone());
            for dep in deps {
                builder = builder.input(dep);
            }
            (name.clone(), builder.build())
        })
        .collect()
}

proptest! {
    /// A graph with no back-edges never produces a cycle, regardless of node
    /// count or edge layout.
    #[test]
    fn forward_edge_graphs_never_report_cycles(
        (count, edges) in (2usize..=8).prop_flat_map(|count| {
            (
                Just(count),
                proptest::collection::vec((0..count, 0..count), 0..=count * 2),
            )
        }),
    ) {
        let targets = make_forward_edge_graph(count, &edges);
        let report = analyse(&targets);
        prop_assert!(report.cycle.is_none(), "unexpected cycle: {:?}", report.cycle);
    }

    /// Any graph containing a back-edge always produces a cycle, even with
    /// extra forward edges layered on top.
    #[test]
    fn back_edge_graphs_always_report_cycles(
        (count, edges) in (2usize..=8).prop_flat_map(|count| {
            (
                Just(count),
                proptest::collection::vec((0..count, 0..count), 0..=count),
            )
        }),
    ) {
        let nodes = sequential_nodes(count);
        // Guarantee a path from the first node to the last by including the
        // chain edges alongside the random forward edges, then close the
        // loop with a back-edge from the last node to the first.
        let mut all_edges = edges;
        all_edges.extend((0..count - 1).map(|i| (i, i + 1)));
        let mut targets = make_forward_edge_graph(count, &all_edges);
        let (Some(last), Some(first)) = (nodes.last(), nodes.first()) else {
            prop_assert!(false, "graph requires at least two nodes");
            return Ok(());
        };
        let builder = EdgeBuilder::new(last.clone()).input(first.clone());
        targets.insert(last.clone(), builder.build());
        let report = analyse(&targets);
        prop_assert!(report.cycle.is_some(), "expected a cycle to be reported");
    }

    /// Missing-dependency records are exactly the edges whose targets are
    /// absent from the target map.
    #[test]
    fn missing_dependencies_match_absent_edge_targets(
        (count, ghost_edges) in (2usize..=6).prop_flat_map(|count| {
            (
                Just(count),
                proptest::collection::hash_set((0..count, 0..3usize), 0..=count),
            )
        }),
    ) {
        let nodes = sequential_nodes(count);
        let mut targets = make_acyclic_chain(&nodes);
        let mut expected: Vec<(camino::Utf8PathBuf, camino::Utf8PathBuf)> = Vec::new();
        for &(node_idx, ghost_idx) in &ghost_edges {
            let ghost = path(&format!("ghost{ghost_idx}"));
            let Some(name) = nodes.get(node_idx).cloned() else {
                prop_assert!(false, "node index out of range");
                return Ok(());
            };
            let Some(edge) = targets.remove(&name) else {
                prop_assert!(false, "node should exist");
                return Ok(());
            };
            let mut builder = EdgeBuilder::new(name.clone()).implicit_dep(ghost.clone());
            for input in edge.inputs {
                builder = builder.input(input);
            }
            for dep in edge.implicit_deps {
                builder = builder.implicit_dep(dep);
            }
            targets.insert(name.clone(), builder.build());
            expected.push((name, ghost));
        }
        let report = analyse(&targets);
        prop_assert!(report.cycle.is_none());
        let mut reported = report.missing_dependencies;
        reported.sort();
        expected.sort();
        for (dependent, missing) in &reported {
            prop_assert!(targets.contains_key(dependent));
            prop_assert!(!targets.contains_key(missing));
        }
        prop_assert_eq!(reported, expected);
    }

    /// Results are stable across arbitrary `HashMap` insertion orderings.
    #[test]
    fn analyse_is_stable_across_insertion_orders(
        (count, order) in (2usize..=6).prop_flat_map(|count| {
            (
                Just(count),
                Just((0..count).collect::<Vec<_>>()).prop_shuffle(),
            )
        }),
    ) {
        let nodes = sequential_nodes(count);
        let canonical = make_cycle_graph(&nodes);
        let baseline = analyse(&canonical);

        let mut shuffled = HashMap::new();
        for index in order {
            let Some(name) = nodes.get(index).cloned() else {
                prop_assert!(false, "node index out of range");
                return Ok(());
            };
            let Some(edge) = canonical.get(&name) else {
                prop_assert!(false, "node should exist");
                return Ok(());
            };
            shuffled.insert(name, edge.clone());
        }
        let report = analyse(&shuffled);
        prop_assert_eq!(report.cycle, baseline.cycle);
        let mut left = report.missing_dependencies;
        let mut right = baseline.missing_dependencies;
        left.sort();
        right.sort();
        prop_assert_eq!(left, right);
    }
}
