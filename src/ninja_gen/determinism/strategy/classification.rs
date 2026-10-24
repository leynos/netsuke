//! Per-case shape counts for generated build-graph specifications.
//!
//! A proptest counter-example is only diagnosable if the printed input says
//! what shape it had. A 50-action, 100-edge specification is tens of kilobytes
//! on one line, so the specification's `Debug` prints these counts instead of
//! the raw vectors, and a failing case can be classified without replaying the
//! seed.
//!
//! The counts are also non-vacuity evidence. Reordering a graph can only be
//! tested on a graph whose classes make reordering observable, so the classes
//! an obligation names are counted here, and a run reporting zero for one of
//! them has not exercised that path.

use crate::ir::{Action, BuildEdge, BuildGraph, DependencyOrder};
use std::collections::BTreeSet;

/// Every combination of `BuildEdge::always` and `Action::restat`, as a mask.
pub(crate) const ALWAYS_RESTAT_ALL: u8 = 0b1111;

/// Shape counts for one [`super::graph::GraphSpec`].
///
/// Every field counts something in the specification's own contents, so the
/// struct is a lossy but stable summary: two specifications with identical
/// counts can still differ, which is why the specification pairs this with a
/// digest rather than printing counts alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Classification {
    /// Number of distinct actions the specification declares.
    pub(crate) actions: usize,
    /// Number of edges the specification declares.
    pub(crate) edges: usize,
    /// Total explicit outputs across every edge.
    pub(crate) explicit_outputs: usize,
    /// Total implicit outputs across every edge.
    pub(crate) implicit_outputs: usize,
    /// Edges declaring more than one explicit output.
    pub(crate) multi_output_edges: usize,
    /// Edges whose action is a dependency-only phony node.
    ///
    /// This is the count that matters to the emitter: a dependency-only action
    /// lowers to a rule-less `phony` build line. An edge's own `phony` field is
    /// never consulted by the emitter at all, so it is counted separately.
    pub(crate) phony_edges: usize,
    /// Edges in serial order carrying more than one implicit dependency.
    pub(crate) dyndep_staged_edges: usize,
    /// Edges marked `always`.
    pub(crate) always_edges: usize,
    /// Edges whose `phony` field is set, whether or not the emitter reads it.
    pub(crate) phony_flag_edges: usize,
    /// Actions marked `restat`.
    pub(crate) restat_actions: usize,
    /// Declared default targets, counting repeats.
    pub(crate) default_targets: usize,
    /// Distinct default targets.
    pub(crate) distinct_default_targets: usize,
    /// Combinations of `BuildEdge::always` and `Action::restat` observed, as a
    /// mask with the bit numbered `2 * always + restat` set.
    ///
    /// `DisplayEdge` emits the edge-level `restat` override only when
    /// `edge.always && !action_restat` (`src/ninja_gen/display_edge.rs:37`), so
    /// a run missing a bit has not exercised that branch's whole truth table.
    /// A mask rather than four flags because a run-wide tally merges by union.
    pub(crate) always_restat_seen: u8,
}

impl Classification {
    /// Count the shapes present in one specification.
    pub(crate) fn of(
        actions: &[(String, Action)],
        edges: &[BuildEdge],
        default_targets: &[camino::Utf8PathBuf],
    ) -> Self {
        // The lookup is a linear scan rather than a map because a specification
        // holds at most fifty actions; a second index would cost more to keep
        // consistent than it saves.
        let action_of = |edge: &BuildEdge| {
            actions
                .iter()
                .find(|(id, _)| id.as_str() == edge.action_id)
                .map(|(_, action)| action)
        };
        let is_dependency_only = |edge: &BuildEdge| {
            action_of(edge).is_some_and(|action| action.recipe.is_dependency_only())
        };
        Self {
            actions: actions.len(),
            edges: edges.len(),
            explicit_outputs: edges.iter().map(|e| e.explicit_outputs.len()).sum(),
            implicit_outputs: edges.iter().map(|e| e.implicit_outputs.len()).sum(),
            multi_output_edges: edges
                .iter()
                .filter(|e| e.explicit_outputs.len() > 1)
                .count(),
            phony_edges: edges.iter().filter(|e| is_dependency_only(e)).count(),
            dyndep_staged_edges: edges
                .iter()
                .filter(|e| {
                    e.dependency_order == DependencyOrder::Serial && e.implicit_deps.len() > 1
                })
                .count(),
            always_edges: edges.iter().filter(|e| e.always).count(),
            phony_flag_edges: edges.iter().filter(|e| e.phony).count(),
            restat_actions: actions.iter().filter(|(_, a)| a.restat).count(),
            default_targets: default_targets.len(),
            distinct_default_targets: default_targets
                .iter()
                .map(|p| p.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            always_restat_seen: edges.iter().fold(0u8, |mask, edge| {
                let restat = action_of(edge).is_some_and(|action| action.restat);
                mask | combination_bit(edge.always, restat)
            }),
        }
    }

    /// Count the shapes present in a materialized graph.
    ///
    /// Used to check that [`super::graph::GraphSpec::materialize`] preserved
    /// what the specification declared, and that two insertion orders of one
    /// specification produced the same graph. Every field is a count or a
    /// mask, so the comparison does not depend on the order the graph's action
    /// map happens to iterate in.
    pub(crate) fn of_graph(graph: &BuildGraph) -> Self {
        let actions: Vec<(String, Action)> = graph
            .actions
            .iter()
            .map(|(id, action)| (id.clone(), action.clone()))
            .collect();
        let edges: Vec<BuildEdge> = graph.edges().cloned().collect();
        Self::of(&actions, &edges, &graph.default_targets)
    }

    /// Whether this specification reaches the class intersection an order
    /// obligation needs.
    ///
    /// The obligation requires multi-output edges, phony edges, dyndep-staged
    /// edges, and non-empty defaults to *coexist in one graph*, not merely to
    /// occur somewhere in a run: each class is a distinct reason for one edge's
    /// line to differ from another's, and a graph carrying only one of them has
    /// reordering to observe between nothing. No single case is guaranteed to
    /// reach the intersection, so callers tally cases rather than asserting
    /// this per case.
    pub(crate) const fn reaches_class_intersection(self) -> bool {
        self.multi_output_edges > 0
            && self.phony_edges > 0
            && self.dyndep_staged_edges > 0
            && self.default_targets > 0
    }
}

/// Whether two edge sequences differ at some position.
///
/// Compares *positions*, not sets: two materializations of one specification
/// hold the same edges in different sequence, so an order property comparing
/// them as sets would be comparing a thing to itself — the vacuity `EP-M0`
/// question 1 found at one key. Content is compared rather than identity
/// because a graph rebuilt from cloned edges has no shared identities to
/// compare.
pub(crate) fn edge_lists_differ(left: &[BuildEdge], right: &[BuildEdge]) -> bool {
    left.len() != right.len()
        || left
            .iter()
            .zip(right)
            .any(|(left_edge, right_edge)| !same_content(left_edge, right_edge))
}

/// The mask bit for one `always`/`restat` combination.
///
/// Spelled with branches rather than a numeric conversion of the two flags:
/// `From` is not callable in a `const fn` on this toolchain, and the arithmetic
/// cast that would work is denied workspace-wide.
const fn combination_bit(always: bool, restat: bool) -> u8 {
    let index = if always { 2 } else { 0 } + if restat { 1 } else { 0 };
    1u8 << index
}

/// Whether two edges describe the same content, ignoring list order.
///
/// Split into the scalar flags and the path vectors rather than written as one
/// nine-clause conjunction. The two halves fail for different reasons — a flag
/// differs because the generator drew a different shape, a path list differs
/// because a permutation was not normalized — and a reader diagnosing a
/// counter-example needs to know which.
fn same_content(left: &BuildEdge, right: &BuildEdge) -> bool {
    same_flags(left, right) && same_path_lists(left, right)
}

/// Whether two edges agree on every scalar field.
fn same_flags(left: &BuildEdge, right: &BuildEdge) -> bool {
    left.action_id == right.action_id
        && left.dependency_order == right.dependency_order
        && left.phony == right.phony
        && left.always == right.always
}

/// Whether two edges agree on every path list, compared as sorted contents.
fn same_path_lists(left: &BuildEdge, right: &BuildEdge) -> bool {
    sorted_paths(&left.explicit_outputs) == sorted_paths(&right.explicit_outputs)
        && sorted_paths(&left.implicit_outputs) == sorted_paths(&right.implicit_outputs)
        && sorted_paths(&left.inputs) == sorted_paths(&right.inputs)
        && sorted_paths(&left.implicit_deps) == sorted_paths(&right.implicit_deps)
        && sorted_paths(&left.order_only_deps) == sorted_paths(&right.order_only_deps)
}

/// A path list's contents, sorted, for order-insensitive comparison.
fn sorted_paths(paths: &[camino::Utf8PathBuf]) -> Vec<String> {
    let mut sorted: Vec<String> = paths.iter().map(|path| path.as_str().to_owned()).collect();
    sorted.sort_unstable();
    sorted
}
