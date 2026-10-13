//! Deterministic ordering of the graph elements Ninja output is built from.
//!
//! Both emission paths — the single-file renderer and the dyndep bundle — walk
//! the same graph in the same order, so the permutations are derived once here.
//! Keeping them pure and shared means a determinism property can be stated over
//! the ordering rules directly, rather than over two hand-copied sort calls that
//! are free to drift apart.

use super::path_key;
use crate::ir::{Action, BuildEdge, BuildGraph};
use camino::Utf8PathBuf;
use itertools::Itertools;

/// Order the graph's edges by their sorted explicit outputs.
///
/// The graph arena preserves insertion order, which follows manifest order and
/// is therefore not a function of the graph's contents. Sorting by the explicit
/// outputs makes the emitted edge order depend only on the graph, and
/// [`path_key`] joins the sorted outputs so an edge carrying several of them has
/// one stable key.
pub(crate) fn ordered_edges(graph: &BuildGraph) -> Vec<&BuildEdge> {
    let mut edges: Vec<&BuildEdge> = graph.edges().collect();
    edges.sort_by_key(|edge| path_key(&edge.explicit_outputs));
    edges
}

/// Order the graph's actions by action ID.
///
/// The action map is hash-backed, so iteration order varies between runs and
/// would otherwise leak into the rule order written to the Ninja file.
pub(crate) fn ordered_actions(graph: &BuildGraph) -> Vec<(&str, &Action)> {
    let mut actions: Vec<(&str, &Action)> = graph
        .actions
        .iter()
        .map(|(id, action)| (id.as_str(), action))
        .collect();
    actions.sort_by_key(|(id, _)| *id);
    actions
}

/// Order the graph's default targets lexicographically.
///
/// Returns owned paths rather than borrows because callers pass the result to
/// [`super::join`], which validates and escapes each path on the way out. Empty
/// when the graph declares none, so callers can use the result to decide whether
/// a `default` statement is emitted at all.
pub(crate) fn ordered_default_targets(graph: &BuildGraph) -> Vec<Utf8PathBuf> {
    graph
        .default_targets
        .iter()
        .sorted()
        .cloned()
        .collect()
}
