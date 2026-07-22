//! Property tests for PR #315 cycle-detection invariants.
//!
//! These generated graph properties verify that `analyse` rejects false
//! positives on DAGs, detects explicit and implicit back-edges, excludes
//! order-only dependencies from traversal, reports missing dependencies, and
//! remains stable across `HashMap` insertion order.

use std::collections::{HashMap, HashSet};

use camino::Utf8PathBuf;
use proptest::prelude::*;

use super::super::{BuildEdge, analyse};

fn node(index: usize) -> Utf8PathBuf {
    Utf8PathBuf::from(format!("n{index}"))
}

fn build_edge(output: Utf8PathBuf) -> BuildEdge {
    BuildEdge {
        action_id: "id".into(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
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

fn dag_from_edges(
    node_count: usize,
    edges: &[(usize, usize, bool)],
) -> HashMap<Utf8PathBuf, BuildEdge> {
    let mut graph = HashMap::new();
    for index in 0..node_count {
        let output = node(index);
        let mut edge = build_edge(output.clone());
        for &(from, to, is_implicit) in edges {
            if from == index && to < from {
                push_dependency(&mut edge, node(to), is_implicit);
            }
        }
        graph.insert(output, edge);
    }
    graph
}

fn dag_strategy() -> impl Strategy<Value = HashMap<Utf8PathBuf, BuildEdge>> {
    (
        1usize..50,
        proptest::collection::vec((0usize..50, 0usize..50, any::<bool>()), 0..250),
    )
        .prop_map(|(node_count, edges)| dag_from_edges(node_count, &edges))
}

fn cyclic_graph_strategy()
-> impl Strategy<Value = (HashMap<Utf8PathBuf, BuildEdge>, Utf8PathBuf, Utf8PathBuf)> {
    (2usize..50, any::<bool>()).prop_map(|(node_count, back_edge_is_implicit)| {
        let chain_edges: Vec<_> = (1..node_count)
            .map(|index| (index, index - 1, false))
            .collect();
        let mut graph = dag_from_edges(node_count, &chain_edges);
        let from = node(0);
        let to = node(node_count - 1);
        let mut edge = build_edge(from.clone());
        push_dependency(&mut edge, to.clone(), back_edge_is_implicit);
        graph.insert(from.clone(), edge);
        (graph, from, to)
    })
}

fn order_only_back_edge_strategy() -> impl Strategy<Value = HashMap<Utf8PathBuf, BuildEdge>> {
    (2usize..50).prop_map(|node_count| {
        let chain_edges: Vec<_> = (1..node_count)
            .map(|index| (index, index - 1, false))
            .collect();
        let mut graph = dag_from_edges(node_count, &chain_edges);
        let root = node(0);
        let mut edge = build_edge(root.clone());
        edge.order_only_deps.push(node(node_count - 1));
        graph.insert(root, edge);
        graph
    })
}

fn missing_graph_strategy() -> impl Strategy<Value = HashMap<Utf8PathBuf, BuildEdge>> {
    (
        dag_strategy(),
        proptest::collection::vec((0usize..50, 0usize..20, any::<bool>()), 1..50),
    )
        .prop_map(|(mut graph, missing_edges)| {
            let node_count = graph.len();
            for (from, missing_index, is_implicit) in missing_edges {
                let bounded_from = from.min(node_count.saturating_sub(1));
                let Some(edge) = graph.get_mut(&node(bounded_from)) else {
                    continue;
                };
                let missing = Utf8PathBuf::from(format!("missing-{missing_index}"));
                push_dependency(edge, missing, is_implicit);
            }
            graph
        })
}

// Accept short, duplicate, or out-of-range order vectors. Chosen entries are
// inserted first, then `or_insert` fills gaps so every original target remains
// present while insertion order still varies.
fn rebuild_in_order(
    graph: &HashMap<Utf8PathBuf, BuildEdge>,
    order: &[usize],
) -> HashMap<Utf8PathBuf, BuildEdge> {
    let mut entries: Vec<_> = graph
        .iter()
        .map(|(key, edge)| (key.clone(), edge.clone()))
        .collect();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let mut rebuilt = HashMap::new();
    for index in order {
        let bounded_index = (*index).min(entries.len().saturating_sub(1));
        let Some((key, edge)) = entries.get(bounded_index) else {
            continue;
        };
        rebuilt.insert(key.clone(), edge.clone());
    }
    for (key, edge) in entries {
        rebuilt.entry(key).or_insert(edge);
    }
    rebuilt
}

fn sorted_missing(report: &super::super::CycleDetectionReport) -> Vec<(Utf8PathBuf, Utf8PathBuf)> {
    let mut missing = report.missing_dependencies.clone();
    missing.sort();
    missing
}

fn injected_missing_deps(graph: &HashMap<Utf8PathBuf, BuildEdge>) -> HashSet<Utf8PathBuf> {
    graph
        .values()
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
            prop_assert!(!graph.contains_key(&dep));
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
