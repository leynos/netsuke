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

use crate::ir::{BuildEdge, BuildGraph};
use blueprint::Blueprint;
use camino::Utf8PathBuf;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use std::hash::{Hash, Hasher};

pub(super) mod blueprint;
pub(super) mod spec;

/// Re-exported so the specification keeps one import path for its callers.
pub(crate) use spec::GraphSpec;

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

/// The generation bounds a graph specification is drawn under.
///
/// The shared domain is the roadmap's, and every obligation but one draws from
/// it. `OBL-NINJA` cannot: it hands each case to a real `ninja` binary twice,
/// so its domain is deliberately smaller, and a duplicate generator would be a
/// second, drifting statement of the same well-formedness argument. The bounds
/// are therefore named once and passed in, with the shared set as the default.
///
/// Only the *sampling* bounds vary. The structural constants — the pool's cost
/// budget, how many outputs an edge may take, the default count — are shared,
/// because they describe what a well-formed specification looks like rather
/// than how large a case is.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GraphBounds {
    /// Fewest actions a specification may declare.
    pub(crate) min_actions: usize,
    /// Greatest number of actions a specification may declare.
    pub(crate) max_actions: usize,
    /// Fewest edges a specification may declare.
    pub(crate) min_edges: usize,
    /// Greatest number of edges a specification may declare.
    pub(crate) max_edges: usize,
    /// Greatest pool slot number an edge may name.
    ///
    /// A drawn slot is clamped to the window of slots allocated before the
    /// edge, so a ceiling much larger than a domain's pool collapses most
    /// dependency draws onto the window's last slot. That is harmless for the
    /// shared domain, whose pools run to 200 paths, but it would leave the
    /// small `OBL-NINJA` domain unable to reach two distinct implicit
    /// dependencies — the shape its dyndep path needs. Each domain therefore
    /// states a ceiling its own pool cannot outgrow.
    pub(crate) max_position: usize,
    /// Fewest leading actions declared dependency-only.
    pub(crate) min_phony_actions: usize,
}

impl GraphBounds {
    /// The bounds every property but `OBL-NINJA` draws under.
    pub(crate) const SHARED: Self = Self {
        min_actions: MIN_ACTIONS,
        max_actions: MAX_ACTIONS,
        min_edges: MIN_EDGES,
        max_edges: MAX_EDGES,
        max_position: MAX_POSITION,
        min_phony_actions: 1,
    };
}

/// The per-edge shape strategy, kept separate so that it reads on its own.
///
/// The action slot is drawn against `action_count` rather than a fixed ceiling,
/// so every drawn slot names a declared action. Bounding it here rather than
/// folding the draw afterwards keeps the mapping total without a wrap-around
/// step, which would make several action slots unreachable for a short list.
///
/// `max_position` comes from the caller's [`GraphBounds`]; see that field's
/// documentation for why the ceiling is domain-specific.
pub(super) fn edge_plan_strategy(
    action_count: usize,
    max_position: usize,
) -> impl Strategy<Value = EdgePlan> {
    (
        (
            0usize..=MAX_EXTRA_PER_EDGE,
            0usize..=1,
            0usize..action_count,
        ),
        (
            prop::collection::vec(0usize..max_position, 0..=3),
            prop::collection::vec(0usize..max_position, 0..=3),
            prop::collection::vec(0usize..max_position, 0..=2),
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

/// Insert one pool edge, reporting a collision as a shrinkable test failure.
///
/// Reported rather than asserted or ignored so that a defect in the generator
/// surfaces as a failure carrying the specification's shape, instead of as a
/// panic shrinking cannot reduce, or as a silently thinner graph that would let
/// a property pass for the wrong reason.
///
/// # Errors
///
/// Returns an error if the edge's outputs collide with one already inserted.
/// The pool's disjoint slices make that unreachable in practice.
fn insert_pool_edge(graph: &mut BuildGraph, edge: &BuildEdge) -> Result<(), TestCaseError> {
    // The arena owns the inserted edge, so its identifier is not needed here.
    graph
        .insert_edge(edge.clone())
        .map_err(|error| TestCaseError::fail(format!("pool slices must not collide: {error}")))?;
    Ok(())
}

/// Hash a path list, its length included so that `[]` and `[""]` differ.
fn hash_paths<H: Hasher>(hasher: &mut H, paths: &[Utf8PathBuf]) {
    paths.len().hash(hasher);
    for path in paths {
        path.as_str().hash(hasher);
    }
}

/// Generate a bounded, well-formed [`GraphSpec`].
///
/// Actions and edges are drawn independently, so the shapes the obligations
/// name are reachable but not guaranteed on any one case. Properties record the
/// classes they observed rather than assuming them.
pub(crate) fn arb_graph_spec() -> impl Strategy<Value = GraphSpec> {
    arb_graph_spec_within(GraphBounds::SHARED)
}

/// Generate a well-formed [`GraphSpec`] inside `bounds`.
///
/// The same construction as [`arb_graph_spec`], with the sampling bounds
/// supplied rather than fixed, so `OBL-NINJA` can draw its smaller domain
/// without a second copy of the well-formedness argument. A domain narrower
/// than the shared one needs at least one executable action and at least one
/// non-phony action for the same reason the shared domain does; the phony
/// share is bounded by `action_count` rather than by a fixed margin, because a
/// one-action domain has no room for the shared margin.
pub(crate) fn arb_graph_spec_within(bounds: GraphBounds) -> impl Strategy<Value = GraphSpec> {
    (
        bounds.min_actions..=bounds.max_actions,
        bounds.min_edges..=bounds.max_edges,
    )
        .prop_flat_map(move |(action_count, edge_count)| {
            let positions: Vec<usize> = (0..edge_count).collect();
            (
                Just(action_count),
                Just(edge_count),
                prop::collection::vec(
                    edge_plan_strategy(action_count, bounds.max_position),
                    edge_count,
                ),
                prop::collection::vec(any::<bool>(), action_count),
                prop::collection::vec(0usize..bounds.max_position, 0..=MAX_DEFAULT_PICKS),
                // At least one action must be executable, so that a non-phony
                // edge has something to name; the phony share stops short of
                // the total and at least two actions exist besides.
                bounds.min_phony_actions
                    ..=(action_count.saturating_sub(2)).max(bounds.min_phony_actions),
                Just(positions).prop_shuffle(),
                // A one-edge domain has no second arrangement, so the rotation
                // is the identity there; the range must still be non-empty,
                // because `1..1` is an empty range and Proptest rejects it.
                1usize..=edge_count.saturating_sub(1).max(1),
            )
        })
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
