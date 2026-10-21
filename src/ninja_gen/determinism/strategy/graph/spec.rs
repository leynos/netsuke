//! A materialized graph specification and its observable shape.
//!
//! This holds the specification type itself, the insertion orders it exposes,
//! and the compact `Debug` that keeps a Proptest counter-example readable. The
//! split from the parent module is the file-size cap rather than a boundary in
//! the reasoning; the generator and its well-formedness argument stay in
//! `mod.rs` and `blueprint.rs`.

use super::{hash_paths, insert_pool_edge};
use crate::ir::{Action, BuildEdge, BuildGraph, DependencyOrder};
use crate::ninja_gen::determinism::strategy::classification::Classification;
use camino::Utf8PathBuf;
use proptest::test_runner::TestCaseError;
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};

/// A bounded, well-formed build-graph specification and two insertion orders.
///
/// Materializing one specification twice under different orders yields two
/// graphs with identical key sets and identical values. That is what makes an
/// insertion-order property sound: a comparison over graphs with *differing*
/// key sets establishes nothing, since any divergence between them would be
/// legitimate and any agreement would be uninformative.
pub(crate) struct GraphSpec {
    /// Action identifier and action, in declaration order.
    pub(super) actions: Vec<(String, Action)>,
    /// The graph's edges, in the order they would be declared.
    pub(super) edges: Vec<BuildEdge>,
    /// Default targets: a sub-multiset of the pool, with repeats.
    pub(super) default_targets: Vec<Utf8PathBuf>,
    /// One permutation of `0..edges.len()`.
    pub(super) first_order: Vec<usize>,
    /// A second permutation, distinct from [`Self::first_order`] whenever the
    /// edge count admits a second arrangement at all.
    pub(super) second_order: Vec<usize>,
}

impl GraphSpec {
    /// Build the graph these parts describe, inserting edges in `order`.
    ///
    /// Edges are cloned from the specification rather than consumed, so one
    /// specification can be materialized as many times as a property needs.
    /// Out-of-range indices in `order` are skipped rather than rejected:
    /// shrinking can shorten a sampled order, and that is not a defect.
    ///
    /// # Errors
    ///
    /// Returns an error if an edge's outputs collide with one already
    /// inserted; see [`insert_pool_edge`] for why that is reported rather than
    /// asserted. The pool's disjoint slices make it unreachable in practice.
    pub(crate) fn materialize(&self, order: &[usize]) -> Result<BuildGraph, TestCaseError> {
        let mut graph = BuildGraph::default();
        for (id, action) in &self.actions {
            graph.actions.insert(id.clone(), action.clone());
        }
        for index in order {
            if let Some(edge) = self.edges.get(*index) {
                insert_pool_edge(&mut graph, edge)?;
            }
        }
        graph.default_targets.clone_from(&self.default_targets);
        Ok(graph)
    }

    /// The specification's insertion orders, as `(first, second)`.
    ///
    /// The two differ whenever the edge count admits a second permutation, so a
    /// property comparing them compares two genuinely different arrangements
    /// rather than a thing to itself.
    pub(crate) fn orders(&self) -> (&[usize], &[usize]) {
        (&self.first_order, &self.second_order)
    }

    /// The number of edges, which bounds a valid index into an insertion order.
    pub(crate) const fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Shape counts for this specification.
    pub(crate) fn classification(&self) -> Classification {
        Classification::of(&self.actions, &self.edges, &self.default_targets)
    }

    /// A stable digest over every field a determinism property can observe.
    ///
    /// There is no `Hash` impl to lean on: `Action` and `BuildEdge` derive
    /// `PartialEq` but not `Hash`, and hashing their `Debug` output would fold
    /// in formatting that is free to change. The walk therefore names each
    /// observable field explicitly.
    fn digest(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        for (id, action) in &self.actions {
            id.hash(&mut hasher);
            action.restat.hash(&mut hasher);
            action.recipe.is_dependency_only().hash(&mut hasher);
        }
        for edge in &self.edges {
            edge.action_id.hash(&mut hasher);
            hash_paths(&mut hasher, &edge.explicit_outputs);
            hash_paths(&mut hasher, &edge.implicit_outputs);
            hash_paths(&mut hasher, &edge.inputs);
            hash_paths(&mut hasher, &edge.implicit_deps);
            hash_paths(&mut hasher, &edge.order_only_deps);
            (edge.dependency_order == DependencyOrder::Serial).hash(&mut hasher);
            edge.phony.hash(&mut hasher);
            edge.always.hash(&mut hasher);
        }
        hash_paths(&mut hasher, &self.default_targets);
        self.first_order.hash(&mut hasher);
        self.second_order.hash(&mut hasher);
        hasher.finish()
    }
}

impl fmt::Debug for GraphSpec {
    /// Print the specification's shape, never its contents.
    ///
    /// Proptest renders a counter-example regardless of the assertion's own
    /// message, and a 50-action, 100-edge specification is tens of kilobytes on
    /// a single line — long enough that the diagnostic a reader needs scrolls
    /// out of reach. The counts say which shapes a case reached and the digest
    /// says whether two runs are looking at the same value, so a failing case
    /// stays diagnosable without replaying its seed.
    ///
    /// The formatter's alternate flag is deliberately ignored. Proptest renders
    /// this value through `{:#?}`, so honouring the flag would pretty-print the
    /// specification's contents across many lines and defeat the compaction the
    /// reader needs; the diagnostic is one line whatever the caller asks for.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "GraphSpec {{ digest: {:016x}, classes: {:?} }}",
            self.digest(),
            self.classification(),
        )
    }
}
