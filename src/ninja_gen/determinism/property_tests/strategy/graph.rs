//! Soundness and non-vacuity properties for `GraphSpec` generation.
//!
//! `EP-M3` delivers infrastructure, so its acceptance is that the generator is
//! sound and non-vacuous rather than that emission is deterministic. Five
//! claims are checked here: the generated shape stays inside the roadmap's
//! bounds; `materialize` round-trips the specification's own counts; the two
//! insertion orders genuinely differ; the compact `Debug` is compact; and the
//! generator reaches every shape class, and every class intersection, an order
//! obligation needs.
//!
//! A separate check is deliberately absent. Proptest's counter-example
//! formatting is only exercised by a failing property, so a test asserting "a
//! failure prints a screen of text" cannot exist without a property designed to
//! fail. That acceptance item is discharged by the compaction of `GraphSpec`'s
//! `Debug` plus the direct bound on the rendered length asserted below.

use crate::ninja_gen::determinism::strategy::classification::edge_lists_differ;
use crate::ninja_gen::determinism::strategy::graph::{
    MAX_ACTIONS, MAX_EDGES, MAX_OUTPUTS, MIN_ACTIONS, MIN_EDGES, arb_graph_spec,
};
use crate::ninja_gen::determinism::strategy::{ALWAYS_RESTAT_ALL, Classification};
use proptest::prelude::*;
use proptest::test_runner::TestRunner;
use std::cell::Cell;

/// A generous ceiling on the rendered `Debug` output, in characters.
///
/// A 50-action, 100-edge specification renders to tens of kilobytes when its
/// vectors are printed field by field. The house diagnostic is a digest and a
/// dozen counters, so a few hundred characters is ample and four thousand is
/// far below the element-by-element dump this guards against.
const MAX_DEBUG_CHARS: usize = 4_000;

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, .. ProptestConfig::default() })]

    #[test]
    fn generated_specs_stay_within_the_roadmap_bounds(spec in arb_graph_spec()) {
        let classes = spec.classification();
        prop_assert!(classes.actions >= MIN_ACTIONS);
        prop_assert!(classes.actions <= MAX_ACTIONS);
        prop_assert!(classes.edges >= MIN_EDGES);
        prop_assert!(classes.edges <= MAX_EDGES);
        prop_assert!(
            classes.explicit_outputs <= MAX_OUTPUTS,
            "explicit outputs {} exceed the cost budget {}",
            classes.explicit_outputs,
            MAX_OUTPUTS,
        );
        prop_assert!(
            classes.explicit_outputs + classes.implicit_outputs <= MAX_OUTPUTS,
            "the pool allocates per edge, so the two kinds share one budget",
        );
    }

    /// The core arm: materializing one specification under two orders must
    /// preserve what it contains, while the two orders must genuinely differ.
    ///
    /// Reordering is only observable when the orders diverge, so the divergence
    /// is asserted rather than assumed. A case whose orders coincided would
    /// compare a thing to itself and establish nothing — the vacuity `EP-M0`
    /// question 1 found at one key.
    #[test]
    fn the_two_orders_preserve_the_specification_while_differing(spec in arb_graph_spec()) {
        let (first, second) = spec.orders();
        let classes = spec.classification();
        let first_graph = materialize(&spec, first)?;
        let second_graph = materialize(&spec, second)?;
        prop_assert_eq!(
            Classification::of_graph(&first_graph),
            classes,
            "materializing under a permutation must not change what the graph contains",
        );
        prop_assert_eq!(
            Classification::of_graph(&second_graph),
            classes,
            "the second materialization must contain what the specification declared",
        );
        prop_assert_eq!(first_graph.actions.len(), classes.actions);
        prop_assert_eq!(
            first_graph.edges().count(),
            classes.edges,
            "every edge is inserted, because the pool's slices are disjoint",
        );
        if spec.edge_count() > 1 {
            prop_assert!(
                edge_lists_differ(&edges_of(&first_graph), &edges_of(&second_graph)),
                "a rotation of a permutation of two or more elements is never that permutation",
            );
        }
    }

    #[test]
    fn the_compact_debug_stays_within_a_screen(spec in arb_graph_spec()) {
        let rendered = format!("{spec:?}");
        prop_assert!(
            rendered.chars().count() <= MAX_DEBUG_CHARS,
            "the rendered Debug is {} characters; the diagnostic must fit on a screen",
            rendered.chars().count(),
        );
        prop_assert!(rendered.contains("digest:"), "the diagnostic must carry the digest");
        prop_assert!(rendered.contains("classes:"), "the diagnostic must carry the class counts");
    }
}

/// Build one materialization, reporting a generator defect as a test failure.
///
/// The error is returned rather than asserted so that a colliding pool reaches
/// proptest as a shrinkable failure carrying the specification's shape, not as
/// a panic shrinking cannot reduce.
fn materialize(
    spec: &crate::ninja_gen::determinism::strategy::graph::GraphSpec,
    order: &[usize],
) -> Result<crate::ir::BuildGraph, TestCaseError> {
    spec.materialize(order).map_err(|error| {
        TestCaseError::fail(format!("the pool's slices must not collide: {error}"))
    })
}

/// A materialized graph's edges, in the arena's sequence.
fn edges_of(graph: &crate::ir::BuildGraph) -> Vec<crate::ir::BuildEdge> {
    graph.edges().cloned().collect()
}

/// The classes a run reached, tallied over its cases.
///
/// The per-class counts are *marginal* maxima: each says the greatest number of
/// that class any single case carried, so on its own a set of healthy marginals
/// cannot show that one case carried them together. [`Self::intersection_cases`]
/// counts the cases that did, which is the only merge that answers the
/// intersection question — the marginals beside it are the diagnostic for a run
/// where it is zero.
#[derive(Clone, Copy, Debug, Default)]
struct Reach {
    /// Greatest number of actions any case declared.
    actions: usize,
    /// Greatest number of edges any case declared.
    edges: usize,
    /// Greatest number of multi-output edges any case declared.
    multi_output_edges: usize,
    /// Greatest number of dependency-only edges any case declared.
    phony_edges: usize,
    /// Greatest number of dyndep-staged edges any case declared.
    dyndep_staged_edges: usize,
    /// Greatest number of default targets any case declared.
    default_targets: usize,
    /// How many cases carried all four intersection classes at once.
    intersection_cases: usize,
    /// Combinations of `always` and `restat` seen, unioned across cases.
    always_restat_seen: u8,
}

impl Reach {
    /// Fold one case's classification into the run's tally.
    fn extended(self, classes: Classification) -> Self {
        Self {
            actions: self.actions.max(classes.actions),
            edges: self.edges.max(classes.edges),
            multi_output_edges: self.multi_output_edges.max(classes.multi_output_edges),
            phony_edges: self.phony_edges.max(classes.phony_edges),
            dyndep_staged_edges: self.dyndep_staged_edges.max(classes.dyndep_staged_edges),
            default_targets: self.default_targets.max(classes.default_targets),
            intersection_cases: self.intersection_cases
                + usize::from(classes.reaches_class_intersection()),
            always_restat_seen: self.always_restat_seen | classes.always_restat_seen,
        }
    }
}

/// Reachability cannot be settled case by case, so measure it once over a run.
///
/// The obligations require classes to coexist in one graph, and a single case
/// need not supply every one of them, so the tally records the intersection
/// separately from the marginals. This is the non-vacuity evidence `EP-M3`
/// owes: a class the generator can never produce would leave every later
/// obligation silently passing.
///
/// The tallies live in a `Cell` because `TestRunner::run` takes an `Fn`.
#[test]
fn the_generator_reaches_every_required_class_across_a_run() {
    let tallies: Cell<Reach> = Cell::new(Reach::default());
    TestRunner::new(ProptestConfig {
        cases: 64,
        ..ProptestConfig::default()
    })
    .run(&arb_graph_spec(), |spec| {
        tallies.set(tallies.get().extended(spec.classification()));
        Ok(())
    })
    .expect("the graph specification should be generatable");

    let observed = tallies.into_inner();
    assert!(
        observed.intersection_cases > 0,
        "no case reached multi-output edges, phony edges, dyndep-staged edges, \
         and non-empty defaults together; reordering has nothing to observe \
         between the classes that did appear: {observed:?}",
    );
    assert_eq!(
        observed.always_restat_seen, ALWAYS_RESTAT_ALL,
        "every combination of edge.always and action.restat must be reachable, \
         because DisplayEdge emits the edge-level restat override only when \
         edge.always && !action.restat: {observed:?}",
    );
}
