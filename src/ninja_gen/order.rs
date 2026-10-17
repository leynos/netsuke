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

/// Order a collection of edges by their sorted explicit outputs.
///
/// The graph arena preserves insertion order, which follows manifest order and
/// is therefore not a function of the graph's contents. Sorting by the explicit
/// outputs makes the emitted edge order depend only on the graph, and
/// [`path_key`] joins the sorted outputs so an edge carrying several of them has
/// one stable key.
///
/// The input is any edge iterator rather than the graph itself, so a
/// determinism property can hand the rule a `Vec` in a constructed order and
/// observe that the result ignores it. Both emission paths pass
/// `graph.edges()`.
pub(crate) fn ordered_edges<'a>(edges: impl Iterator<Item = &'a BuildEdge>) -> Vec<&'a BuildEdge> {
    let mut collected: Vec<&BuildEdge> = edges.collect();
    collected.sort_by_key(|edge| path_key(&edge.explicit_outputs));
    collected
}

/// Order a collection of actions by action ID.
///
/// The action map is hash-backed, so iteration order varies between runs and
/// would otherwise leak into the rule order written to the Ninja file.
///
/// The input is any `(id, action)` iterator for the same reason as
/// [`ordered_edges`]: the sort is the property under test, and accepting the
/// graph would make the unsorted arrangement unexpressible.
pub(crate) fn ordered_actions<'a>(
    actions: impl Iterator<Item = (&'a str, &'a Action)>,
) -> Vec<(&'a str, &'a Action)> {
    let mut collected: Vec<(&str, &Action)> = actions.collect();
    collected.sort_by_key(|(id, _)| *id);
    collected
}

/// Order the graph's default targets lexicographically.
///
/// Returns owned paths rather than borrows because callers pass the result to
/// [`super::join`], which validates and escapes each path on the way out. Empty
/// when the graph declares none, so callers can use the result to decide whether
/// a `default` statement is emitted at all.
pub(crate) fn ordered_default_targets(graph: &BuildGraph) -> Vec<Utf8PathBuf> {
    graph.default_targets.iter().sorted().cloned().collect()
}
