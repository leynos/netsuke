//! A bounded, well-formed build-graph specification and its generator.
//!
//! Determinism properties need a graph whose *contents* can be held fixed while
//! its *insertion order* is varied, which a `BuildGraph` cannot express once it
//! has been built: the arena remembers only the order it was given.
//! [`GraphSpec`] therefore holds a graph's parts unresolved, and
//! [`GraphSpec::materialize`] builds a real [`BuildGraph`] from them in a
//! caller-chosen permutation.
//!
//! Well-formedness is established by construction rather than by filtering.
//! Rejection sampling would be the obvious alternative, but a generator that
//! discards its own output is exactly the vacuity defect these properties exist
//! to catch: the run would report success while never reaching the shapes the
//! obligations name. Three structural rules carry the guarantee. Output paths
//! come from one pre-numbered pool handed to edges as contiguous,
//! non-overlapping slices, so no two edges can claim the same output. Action
//! identifiers are indexed rather than hashed, so every edge's action exists.
//! An edge's inputs and dependencies are drawn only from pool slots allocated
//! before it, so the graph is acyclic by construction, and the missing-
//! dependency path stays exercised by the generated external paths.
//!
//! [`blueprint`] holds the resolution step that turns drawn values into a
//! specification; the split is the file-size cap, not a boundary in the
//! reasoning.

use super::classification::Classification;
use crate::ir::{Action, BuildEdge, BuildGraph, DependencyOrder};
use blueprint::Blueprint;
use camino::Utf8PathBuf;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};

pub(super) mod blueprint;

/// Greatest number of actions a specification may declare.
pub(crate) const MAX_ACTIONS: usize = 50;
/// Fewest actions a specification declares.
///
/// Five is a sampling threshold rather than a correctness bound. Independently
/// drawn insertion orders of a short key set coincide far too often to be
/// informative — measured at 23 of 200 two-key trials exhausting eight attempts
/// and 200 of 200 at one key — so a domain admitting fewer than five keys
/// spends most of its budget on cases that cannot diverge. See `OBL-ORDER`.
pub(crate) const MIN_ACTIONS: usize = 5;
/// Greatest number of edges a specification may declare.
pub(crate) const MAX_EDGES: usize = 100;
/// Fewest edges a specification declares; see [`MIN_ACTIONS`].
pub(crate) const MIN_EDGES: usize = 5;
/// Greatest number of pool paths a specification allocates, all kinds together.
///
/// This is a cost budget, not a correctness bound: the per-case cost table the
/// properties were sized against was measured under it. See `Constraint 10` and
/// `ADR-030` amendment 1.
pub(crate) const MAX_OUTPUTS: usize = 200;
/// Greatest number of outputs beyond the first that one edge receives.
pub(super) const MAX_EXTRA_PER_EDGE: usize = 3;
/// Greatest number of external dependency paths one edge receives.
pub(super) const MAX_EXTERNAL_PER_EDGE: usize = 2;
/// Greatest number of default targets a specification declares.
pub(super) const MAX_DEFAULT_PICKS: usize = 6;
/// Upper bound for a drawn pool slot number.
pub(super) const MAX_POSITION: usize = 200;
/// Namespace for pool outputs. Deliberately clear of `.netsuke/`, so
/// `reject_reserved_paths` never fires on a generated graph.
pub(super) const OUTPUT_PREFIX: &str = "out";
/// Namespace for generated external dependencies, which no edge produces.
pub(super) const EXTERNAL_PREFIX: &str = "ext";

/// One edge's shape decisions, before any path is allocated.
///
/// The position vectors hold *pool slot numbers* rather than paths: the pool
/// does not exist until every edge's share is known, so a plan can only name
/// slots, and resolution happens once at build time. An edge's action is chosen
/// the same way, by slot, so that "is this edge a phony node?" is answered by
/// the action it names rather than by a flag free to disagree with it.
#[derive(Debug, Clone)]
pub(super) struct EdgePlan {
    /// Output slots requested beyond the mandatory first one.
    pub(super) extra_outputs: usize,
    /// Output slots requested for implicit outputs.
    pub(super) implicit_outputs: usize,
    /// Requested action slot.
    pub(super) action_position: usize,
    /// Requested input slots.
    pub(super) input_positions: Vec<usize>,
    /// Requested implicit-dependency slots.
    pub(super) implicit_positions: Vec<usize>,
    /// Requested order-only-dependency slots.
    pub(super) order_only_positions: Vec<usize>,
    /// Number of external dependency paths to append.
    pub(super) external_inputs: usize,
    /// Whether the edge's dependency list is in serial order.
    pub(super) is_serial: bool,
    /// Whether the edge is marked `always`.
    pub(super) is_always: bool,
    /// Whether the edge's `phony` field is set. The emitter ignores it; it is
    /// generated so that the two `always`/`restat` and phony combinations the
    /// obligations name all occur.
    pub(super) is_phony: bool,
}

/// The per-edge shape strategy, kept separate so that it reads on its own.
///
/// The action slot is drawn against `action_count` rather than a fixed ceiling,
/// so every drawn slot names a declared action. Bounding it here rather than
/// folding the draw afterwards keeps the mapping total without a wrap-around
/// step, which would make several action slots unreachable for a short list.
pub(super) fn edge_plan_strategy(action_count: usize) -> impl Strategy<Value = EdgePlan> {
    (
        (
            0usize..=MAX_EXTRA_PER_EDGE,
            0usize..=1,
            0usize..action_count,
        ),
        (
            prop::collection::vec(0usize..MAX_POSITION, 0..=3),
            prop::collection::vec(0usize..MAX_POSITION, 0..=3),
            prop::collection::vec(0usize..MAX_POSITION, 0..=2),
        ),
        (
            0usize..=MAX_EXTERNAL_PER_EDGE,
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
        ),
    )
        .prop_map(
            |(
                (extra_outputs, implicit_outputs, action_position),
                (input_positions, implicit_positions, order_only_positions),
                (external_inputs, is_serial, is_always, is_phony),
            )| EdgePlan {
                extra_outputs,
                implicit_outputs,
                action_position,
                input_positions,
                implicit_positions,
                order_only_positions,
                external_inputs,
                is_serial,
                is_always,
                is_phony,
            },
        )
}

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
    /// inserted. The pool's disjoint slices make that unreachable in practice;
    /// it is reported rather than asserted or ignored so that a defect in the
    /// generator surfaces as a shrinkable test failure carrying the offending
    /// specification's shape, instead of as a panic that shrinking cannot
    /// reduce, or as a silently thinner graph that would let a property pass
    /// for the wrong reason.
    pub(crate) fn materialize(&self, order: &[usize]) -> Result<BuildGraph, TestCaseError> {
        let mut graph = BuildGraph::default();
        for (id, action) in &self.actions {
            graph.actions.insert(id.clone(), action.clone());
        }
        for index in order {
            if let Some(edge) = self.edges.get(*index) {
                graph.insert_edge(edge.clone()).map_err(|error| {
                    TestCaseError::fail(format!("pool slices must not collide: {error}"))
                })?;
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

/// Hash a path list, its length included so that `[]` and `[""]` differ.
fn hash_paths<H: Hasher>(hasher: &mut H, paths: &[Utf8PathBuf]) {
    paths.len().hash(hasher);
    for path in paths {
        path.as_str().hash(hasher);
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

/// Generate a bounded, well-formed [`GraphSpec`].
///
/// Actions and edges are drawn independently, so the shapes the obligations
/// name are reachable but not guaranteed on any one case. Properties record the
/// classes they observed rather than assuming them.
pub(crate) fn arb_graph_spec() -> impl Strategy<Value = GraphSpec> {
    (MIN_ACTIONS..=MAX_ACTIONS, MIN_EDGES..=MAX_EDGES).prop_flat_map(
        |(action_count, edge_count)| {
            let positions: Vec<usize> = (0..edge_count).collect();
            (
                Just(action_count),
                Just(edge_count),
                prop::collection::vec(edge_plan_strategy(action_count), edge_count),
                prop::collection::vec(any::<bool>(), action_count),
                prop::collection::vec(0usize..MAX_POSITION, 0..=MAX_DEFAULT_PICKS),
                // At least one action must be executable, so that a non-phony
                // edge has something to name; the phony share stops short of
                // the total and at least two actions exist besides.
                1usize..=(action_count.saturating_sub(2)).max(1),
                Just(positions).prop_shuffle(),
                1usize..edge_count,
            )
        },
    )
    .prop_map(
        |(
            action_count,
            edge_count,
            plans,
            restat_flags,
            default_picks,
            phony_count,
            shuffled,
            rotation,
        )| {
            Blueprint {
                action_count,
                edge_count,
                plans,
                restat_flags,
                default_picks,
                phony_count,
                shuffled,
                rotation,
            }
            .build()
        },
    )
}

/// Rotate a permutation by `offset`, wrapping, and never returning it unchanged.
///
/// Rotation obtains the second insertion order because it is *guaranteed* to
/// differ from the first for any permutation of two or more elements and any
/// offset in `1..len`. Independently drawing the two orders was measured to
/// coincide far too often to be usable, and redrawing until they differ spends
/// the case budget rejecting valid samples. A one-element permutation has no
/// alternative arrangement, so the rotation is the identity there and no
/// property may assume otherwise.
pub(super) fn rotate(order: &[usize], offset: usize) -> Vec<usize> {
    let Some(last_index) = order.len().checked_sub(1) else {
        return order.to_vec();
    };
    let mut rotated = order.to_vec();
    if last_index > 0 {
        rotated.rotate_left(offset.min(last_index).max(1));
    }
    rotated
}
