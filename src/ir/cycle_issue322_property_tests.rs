//! Property tests for PR #315 cycle-detection invariants.
//!
//! These generated graph properties verify that `analyse` rejects false
//! positives on DAGs, detects explicit and implicit back-edges, excludes
//! order-only dependencies from traversal, reports missing dependencies, and
//! remains stable across `HashMap` insertion order.

use std::collections::HashSet;

use camino::Utf8PathBuf;
use proptest::prelude::*;

use super::super::super::graph::{BuildEdge, BuildGraph};
use super::super::{CycleDetectionReport, analyse};

fn node(index: usize) -> Utf8PathBuf {
    Utf8PathBuf::from(format!("n{index}"))
}

fn build_edge(output: Utf8PathBuf) -> BuildEdge {
    BuildEdge {
        action_id: "id".into(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        dependency_order: crate::ir::DependencyOrder::Parallel,
        explicit_outputs: vec![output],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    }
}

fn push_dependency(edge: &mut BuildEdge, dep: Utf8PathBuf, is_implicit: bool) {
    if is_implicit {
        edge.implicit_deps.push(dep);
    } else {
        edge.inputs.push(dep);
    }
}

fn dag_from_edges(node_count: usize, edges: &[(usize, usize, bool)]) -> BuildGraph {
    let mut graph = BuildGraph::default();
    for index in 0..node_count {
        let output = node(index);
        let mut edge = build_edge(output.clone());
        for &(from, to, is_implicit) in edges {
            if from == index && to < from {
                push_dependency(&mut edge, node(to), is_implicit);
            }
        }
        assert!(
            graph.insert_edge(edge).is_ok(),
            "test graph output aliases must be unique",
        );
    }
    graph
}

fn dag_strategy() -> impl Strategy<Value = BuildGraph> {
    (
        1usize..50,
        proptest::collection::vec((0usize..50, 0usize..50, any::<bool>()), 0..250),
    )
        .prop_map(|(node_count, edges)| dag_from_edges(node_count, &edges))
}

/// Build a linear graph and replace `n0` with an edge back to its final node.
fn graph_with_back_edge(
    node_count: usize,
    add_back_edge: impl FnOnce(&mut BuildEdge, Utf8PathBuf),
) -> (BuildGraph, Utf8PathBuf, Utf8PathBuf) {
    let chain_edges: Vec<_> = (1..node_count)
        .map(|index| (index, index - 1, false))
        .collect();
    let mut graph = dag_from_edges(node_count, &chain_edges);
    let from = node(0);
    let to = node(node_count - 1);
    let mut edge = build_edge(from.clone());
    add_back_edge(&mut edge, to.clone());
    assert!(
        graph.replace_edge_for_output(from.as_path(), edge),
        "the generated graph must contain n0",
    );
    (graph, from, to)
}

fn cyclic_graph_strategy() -> impl Strategy<Value = (BuildGraph, Utf8PathBuf, Utf8PathBuf)> {
    (2usize..50, any::<bool>()).prop_map(|(node_count, back_edge_is_implicit)| {
        graph_with_back_edge(node_count, |edge, to| {
            push_dependency(edge, to, back_edge_is_implicit);
        })
    })
}

fn order_only_back_edge_strategy() -> impl Strategy<Value = BuildGraph> {
    (2usize..50).prop_map(|node_count| {
        graph_with_back_edge(node_count, |edge, to| {
            edge.order_only_deps.push(to);
        })
        .0
    })
}

fn missing_graph_strategy() -> impl Strategy<Value = BuildGraph> {
    (
        dag_strategy(),
        proptest::collection::vec((0usize..50, 0usize..20, any::<bool>()), 1..50),
    )
        .prop_map(|(mut graph, missing_edges)| {
            let node_count = graph.edge_count();
            for (from, missing_index, is_implicit) in missing_edges {
                let bounded_from = from.min(node_count.saturating_sub(1));
                let output = node(bounded_from);
                let Some((_, edge)) = graph.target_for_output(output.as_path()) else {
                    continue;
                };
                let missing = Utf8PathBuf::from(format!("missing-{missing_index}"));
                let mut replacement = edge.clone();
                push_dependency(&mut replacement, missing, is_implicit);
                let _ = graph.replace_edge_for_output(output.as_path(), replacement);
            }
            graph
        })
}

// Accept short, duplicate, or out-of-range order vectors. Chosen entries are
// inserted first, then `or_insert` fills gaps so every original target remains
// present while insertion order still varies.
fn rebuild_in_order(graph: &BuildGraph, order: &[usize]) -> BuildGraph {
    let mut entries: Vec<_> = graph.edges().cloned().collect();
    entries.sort_by(|left, right| {
        left.explicit_outputs
            .first()
            .cmp(&right.explicit_outputs.first())
    });
    let mut rebuilt = BuildGraph::default();
    for index in order {
        let bounded_index = (*index).min(entries.len().saturating_sub(1));
        let Some(edge) = entries.get(bounded_index) else {
            continue;
        };
        let Some(output) = edge.explicit_outputs.first() else {
            continue;
        };
        if rebuilt.target_for_output(output.as_path()).is_none() {
            assert!(
                rebuilt.insert_edge(edge.clone()).is_ok(),
                "test graph output aliases must be unique",
            );
        }
    }
    for edge in entries {
        let Some(output) = edge.explicit_outputs.first() else {
            continue;
        };
        if rebuilt.target_for_output(output.as_path()).is_none() {
            assert!(
                rebuilt.insert_edge(edge).is_ok(),
                "test graph output aliases must be unique",
            );
        }
    }
    rebuilt
}

fn sorted_missing(report: &CycleDetectionReport) -> Vec<(Utf8PathBuf, Utf8PathBuf)> {
    let mut missing = report.missing_dependencies.clone();
    missing.sort();
    missing
}

fn injected_missing_deps(graph: &BuildGraph) -> HashSet<Utf8PathBuf> {
    graph
        .edges()
        .flat_map(|edge| edge.inputs.iter().chain(&edge.implicit_deps))
        .filter(|dep| dep.as_str().starts_with("missing-"))
        .cloned()
        .collect()
}

proptest! {
    #[test]
    fn generated_dag_has_no_cycle(graph in dag_strategy()) {
        prop_assert!(analyse(&graph).cycle.is_none());
    }

    #[test]
    fn generated_back_edge_produces_cycle((graph, from, to) in cyclic_graph_strategy()) {
        let cycle = analyse(&graph).cycle.expect("back-edge should produce cycle");
        let cycle_edges: HashSet<_> = cycle
            .windows(2)
            .filter_map(|pair| {
                let [left, right] = pair else {
                    return None;
                };
                Some((left.clone(), right.clone()))
            })
            .collect();
        prop_assert!(cycle_edges.contains(&(from, to)));
    }

    #[test]
    fn order_only_back_edge_has_no_cycle(graph in order_only_back_edge_strategy()) {
        prop_assert!(analyse(&graph).cycle.is_none());
    }

    #[test]
    fn generated_missing_dependencies_are_absent_targets(graph in missing_graph_strategy()) {
        let injected_missing = injected_missing_deps(&graph);
        let report = analyse(&graph);
        let reported_missing: HashSet<_> = report
            .missing_dependencies
            .iter()
            .map(|(_, dep)| dep.clone())
            .collect();

        prop_assert_eq!(&reported_missing, &injected_missing);
        for (_, dep) in report.missing_dependencies {
            prop_assert!(graph.target_for_output(dep.as_path()).is_none());
        }
    }

    #[test]
    fn generated_dag_results_are_stable_across_insertion_orders(graph in missing_graph_strategy(), order in proptest::collection::vec(0usize..50, 0..100)) {
        let baseline = analyse(&graph);
        let reordered = rebuild_in_order(&graph, &order);
        let reordered_report = analyse(&reordered);
        prop_assert_eq!(&baseline.cycle, &reordered_report.cycle);
        prop_assert_eq!(sorted_missing(&baseline), sorted_missing(&reordered_report));
    }

    #[test]
    fn generated_cycle_results_are_stable_across_insertion_orders((graph, _, _) in cyclic_graph_strategy(), order in proptest::collection::vec(0usize..50, 0..100)) {
        let baseline = analyse(&graph);
        let reordered = rebuild_in_order(&graph, &order);
        let reordered_report = analyse(&reordered);

        prop_assert_eq!(&baseline.cycle, &reordered_report.cycle);
        prop_assert_eq!(sorted_missing(&baseline), sorted_missing(&reordered_report));
    }
}
