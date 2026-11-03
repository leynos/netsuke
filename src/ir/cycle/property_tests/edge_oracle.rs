//! An independent edge-level cycle oracle for generated graphs.
//!
//! [`super::participation_tests`] compares the alias traversal in
//! `crate::ir::cycle::analyse` against this oracle. The oracle derives its
//! verdict from edge identifiers and the arcs between them alone, so the two
//! sides share no traversal code and agreement is evidence rather than a
//! restatement of the detector.

use std::collections::{HashMap, HashSet, VecDeque};

use super::super::super::graph::{BuildGraph, EdgeId};

/// Check for a cycle in the edge graph using Kahn's topological sort.
pub(super) fn edge_graph_has_cycle(graph: &BuildGraph) -> bool {
    let edge_ids = indexed_edge_ids(graph);
    let (indegrees, successors) = edge_graph_adjacency(graph, &edge_ids);
    kahn_reports_cycle(edge_ids.len(), indegrees, &successors)
}

/// Map each generated edge's primary output to its stable graph identifier.
fn indexed_edge_ids(graph: &BuildGraph) -> Vec<EdgeId> {
    graph
        .edges()
        .filter_map(|edge| edge.explicit_outputs.first())
        .filter_map(|output| graph.edge_id_for_output(output.as_path()))
        .collect()
}

/// Build successor sets and in-degrees for the independent edge-level oracle.
fn edge_graph_adjacency(
    graph: &BuildGraph,
    edge_ids: &[EdgeId],
) -> (HashMap<EdgeId, usize>, HashMap<EdgeId, HashSet<EdgeId>>) {
    let mut indegrees: HashMap<EdgeId, usize> =
        edge_ids.iter().copied().map(|id| (id, 0)).collect();
    let mut successors: HashMap<EdgeId, HashSet<EdgeId>> = edge_ids
        .iter()
        .copied()
        .map(|id| (id, HashSet::new()))
        .collect();

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
                .is_some_and(|set| set.insert(target));
            if inserted {
                increment_indegree(&mut indegrees, target);
            }
        }
    }

    (indegrees, successors)
}

/// Count one incoming arc for `target`, ignoring identifiers the map omits.
fn increment_indegree(indegrees: &mut HashMap<EdgeId, usize>, target: EdgeId) {
    if let Some(degree) = indegrees.get_mut(&target) {
        *degree += 1;
    }
}

/// Return whether Kahn's queue leaves any edge in the graph.
fn kahn_reports_cycle(
    edge_count: usize,
    mut indegrees: HashMap<EdgeId, usize>,
    successors: &HashMap<EdgeId, HashSet<EdgeId>>,
) -> bool {
    let mut ready = indegrees
        .iter()
        .filter_map(|(edge_id, degree)| (*degree == 0).then_some(*edge_id))
        .collect::<VecDeque<_>>();
    let mut removed = 0;
    while let Some(edge_id) = ready.pop_front() {
        removed += 1;
        release_successors(edge_id, &mut indegrees, successors, &mut ready);
    }

    removed != edge_count
}

/// Queue every successor whose last incoming arc was just released.
fn release_successors(
    edge_id: EdgeId,
    indegrees: &mut HashMap<EdgeId, usize>,
    successors: &HashMap<EdgeId, HashSet<EdgeId>>,
    ready: &mut VecDeque<EdgeId>,
) {
    let Some(dependents) = successors.get(&edge_id) else {
        return;
    };
    for successor in dependents {
        let Some(degree) = indegrees.get_mut(successor) else {
            continue;
        };
        *degree = degree.saturating_sub(1);
        if *degree == 0 {
            ready.push_back(*successor);
        }
    }
}
