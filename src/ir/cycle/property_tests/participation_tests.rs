//! Compare alias traversal with an independent edge-level cycle oracle.

use std::collections::{HashMap, HashSet, VecDeque};

use camino::Utf8PathBuf;
use proptest::prelude::*;

use super::super::super::graph::{BuildEdge, BuildGraph};
use super::super::analyse;

#[derive(Clone, Copy, Debug)]
enum DependencyKind {
    Input,
    Implicit,
    OrderOnly,
}

#[derive(Clone, Copy, Debug)]
enum BackArcKind {
    None,
    Input,
    Implicit,
    OrderOnly,
    ImplicitOutput,
}

/// Generate a dependency field for the acyclic base chain.
fn dependency_kind_strategy() -> impl Strategy<Value = DependencyKind> {
    prop_oneof![
        Just(DependencyKind::Input),
        Just(DependencyKind::Implicit),
        Just(DependencyKind::OrderOnly),
    ]
}

/// Generate an acyclic graph or a cycle closed by each supported arc class.
fn back_arc_kind_strategy() -> impl Strategy<Value = BackArcKind> {
    prop_oneof![
        Just(BackArcKind::None),
        Just(BackArcKind::Input),
        Just(BackArcKind::Implicit),
        Just(BackArcKind::OrderOnly),
        Just(BackArcKind::ImplicitOutput),
    ]
}

/// Generate graphs with one to three output aliases on every edge.
fn participation_graph_strategy() -> impl Strategy<Value = BuildGraph> {
    (2usize..=12)
        .prop_flat_map(|edge_count| {
            (
                Just(edge_count),
                proptest::collection::vec(
                    dependency_kind_strategy(),
                    edge_count - 1..=edge_count - 1,
                ),
                proptest::collection::vec(0usize..=2, edge_count..=edge_count),
                back_arc_kind_strategy(),
            )
        })
        .prop_map(|(edge_count, chain_kinds, mut implicit_counts, back_arc)| {
            if let Some(first) = implicit_counts.first_mut() {
                *first = (*first).max(1);
            }
            if let Some(last) = implicit_counts.last_mut() {
                *last = (*last).max(1);
            }
            build_participation_graph(edge_count, &chain_kinds, &implicit_counts, back_arc)
        })
}

/// Build an acyclic chain and optionally close it with a selected back arc.
fn build_participation_graph(
    edge_count: usize,
    chain_kinds: &[DependencyKind],
    implicit_counts: &[usize],
    back_arc: BackArcKind,
) -> BuildGraph {
    let mut edges = implicit_counts
        .iter()
        .copied()
        .enumerate()
        .map(|(index, implicit_count)| build_edge(index, implicit_count))
        .collect::<Vec<_>>();

    for (previous_index, (edge, kind)) in edges
        .iter_mut()
        .skip(1)
        .zip(chain_kinds.iter().copied())
        .enumerate()
    {
        push_dependency(edge, explicit_output(previous_index), kind);
    }

    let final_index = edge_count - 1;
    let closing_arc = match back_arc {
        BackArcKind::None => None,
        BackArcKind::Input => Some((explicit_output(final_index), DependencyKind::Input)),
        BackArcKind::Implicit => Some((explicit_output(final_index), DependencyKind::Implicit)),
        BackArcKind::OrderOnly => Some((explicit_output(final_index), DependencyKind::OrderOnly)),
        BackArcKind::ImplicitOutput => {
            Some((implicit_output(final_index, 0), DependencyKind::Input))
        }
    };
    if let Some((dependency, kind)) = closing_arc {
        let first_edge = edges
            .first_mut()
            .expect("generated graphs have at least two edges");
        push_dependency(first_edge, dependency, kind);
    }

    let mut graph = BuildGraph::default();
    for edge in edges {
        graph
            .insert_edge(edge)
            .expect("generated output aliases must be unique");
    }
    graph
}

/// Build one edge with a primary explicit output and generated aliases.
fn build_edge(index: usize, implicit_count: usize) -> BuildEdge {
    BuildEdge {
        action_id: format!("edge-{index}"),
        progress_label: None,
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        dependency_order: crate::ir::DependencyOrder::Parallel,
        explicit_outputs: vec![explicit_output(index)],
        implicit_outputs: (0..implicit_count)
            .map(|alias| implicit_output(index, alias))
            .collect(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    }
}

/// Return the primary explicit output for one edge.
fn explicit_output(index: usize) -> Utf8PathBuf {
    Utf8PathBuf::from(format!("n{index}"))
}

/// Return one implicit output alias for an edge.
fn implicit_output(index: usize, alias: usize) -> Utf8PathBuf {
    Utf8PathBuf::from(format!("n{index}.extra{alias}"))
}

/// Store `dependency` in the selected dependency field.
fn push_dependency(edge: &mut BuildEdge, dependency: Utf8PathBuf, kind: DependencyKind) {
    match kind {
        DependencyKind::Input => edge.inputs.push(dependency),
        DependencyKind::Implicit => edge.implicit_deps.push(dependency),
        DependencyKind::OrderOnly => edge.order_only_deps.push(dependency),
    }
}

/// Check for a cycle in the edge graph using Kahn's topological sort.
fn edge_graph_has_cycle(graph: &BuildGraph) -> bool {
    let edge_ids = graph
        .edges()
        .map(|edge| {
            let output = edge
                .explicit_outputs
                .first()
                .expect("generated edges have an explicit output");
            graph
                .edge_id_for_output(output.as_path())
                .expect("every generated output alias is indexed")
        })
        .collect::<Vec<_>>();
    let mut indegrees = edge_ids
        .iter()
        .copied()
        .map(|edge_id| (edge_id, 0usize))
        .collect::<HashMap<_, _>>();
    let mut successors = edge_ids
        .iter()
        .copied()
        .map(|edge_id| (edge_id, HashSet::new()))
        .collect::<HashMap<_, _>>();

    for (edge, source) in graph.edges().zip(edge_ids.iter().copied()) {
        let dependencies = edge
            .inputs
            .iter()
            .chain(&edge.implicit_deps)
            .chain(&edge.order_only_deps);
        for dependency in dependencies {
            let Some(target) = graph.edge_id_for_output(dependency.as_path()) else {
                continue;
            };
            let inserted = successors
                .get_mut(&source)
                .expect("every edge has a successor set")
                .insert(target);
            if inserted {
                *indegrees
                    .get_mut(&target)
                    .expect("every dependency edge has an in-degree") += 1;
            }
        }
    }

    let mut ready = indegrees
        .iter()
        .filter_map(|(edge_id, degree)| (*degree == 0).then_some(*edge_id))
        .collect::<VecDeque<_>>();
    let mut removed = 0;
    while let Some(edge_id) = ready.pop_front() {
        removed += 1;
        for successor in successors
            .get(&edge_id)
            .expect("every edge has a successor set")
        {
            let degree = indegrees
                .get_mut(successor)
                .expect("every successor has an in-degree");
            *degree -= 1;
            if *degree == 0 {
                ready.push_back(*successor);
            }
        }
    }

    removed != edge_ids.len()
}

proptest! {
    #[test]
    fn alias_traversal_agrees_with_edge_oracle(graph in participation_graph_strategy()) {
        prop_assert_eq!(analyse(&graph).cycle.is_some(), edge_graph_has_cycle(&graph));
    }
}
