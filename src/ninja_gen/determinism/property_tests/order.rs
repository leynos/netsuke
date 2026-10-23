//! `OBL-ORDER`: emission does not depend on insertion or iteration order.
//!
//! The obligation is split into two arms because a single one is not
//! seed-reproducible.
//!
//! The *core* arm hands [`ordered_edges`] and [`ordered_actions`] a `Vec` whose
//! two arrangements are **constructed**: the second is a rotation of the first,
//! checked to differ. No sampling decides whether the case is perturbed, so the
//! arm cannot go quiet and shrinking stays exact.
//!
//! The *end-to-end* arm materializes one [`GraphSpec`] twice under its two
//! insertion orders and compares the whole emitted bundle. The two graphs hold
//! identical key sets and identical values, so any divergence is an ordering
//! defect rather than a difference in the graphs. `HashMap`'s per-process
//! `RandomState` can still hand both graphs the same iteration order, in which
//! case the case says nothing: the arm therefore tallies the cases whose orders
//! genuinely differed and fails when that tally is zero, rather than reporting
//! a pass over identical inputs. The tally is what `EP-M0` question 1 found
//! missing at one key, where no alternative order exists at all.

use super::support::{coverage_runner, materialize};
use crate::ir::BuildEdge;
use crate::ninja_gen::determinism::strategy::classification::{
    ALWAYS_RESTAT_ALL, Classification, edge_lists_differ,
};
use crate::ninja_gen::determinism::strategy::graph::{GraphSpec, arb_graph_spec};
use crate::ninja_gen::generate_bundle;
use crate::ninja_gen::order::{ordered_actions, ordered_edges};
use crate::ninja_gen::path_key;
use proptest::prelude::*;

use super::support::HEAVY_SHRINK_TIME_MS;
use proptest::test_runner::TestCaseError;
use std::cell::Cell;

proptest! {
    // Emission is string building, but a case can hold 100 edges and 200 pool
    // paths, so the budget sits below the key suite's.
    #![proptest_config(ProptestConfig {
        cases: 128,
        max_shrink_time: HEAVY_SHRINK_TIME_MS,
        ..ProptestConfig::default()
    })]

    /// The edge order rule ignores the order the edges arrive in.
    ///
    /// The perturbation is a constructed rotation, so this case demonstrably
    /// compares two arrangements rather than hoping a shuffle moved something.
    /// The rule is the one both emission paths call, so this is where
    /// `MUT-EDGESORT` stops being invisible.
    #[test]
    fn ordered_edges_ignore_input_order(spec in arb_graph_spec()) {
        let (first, _) = spec.orders();
        let graph = materialize(&spec, first)?;
        let edges: Vec<&BuildEdge> = graph.edges().collect();
        let rotated = rotated(&edges, 3);

        prop_assert!(
            edge_keys(&edges) != edge_keys(&rotated),
            "the constructed rotation must actually perturb the input order",
        );
        prop_assert_eq!(
            edge_keys(&ordered_edges(edges.iter().copied())),
            edge_keys(&ordered_edges(rotated.iter().copied())),
            "the rule must ignore the order its edges arrive in",
        );
    }

    /// The action order rule ignores the order the actions arrive in.
    ///
    /// The same constructed perturbation, over one graph's action map.
    /// `MUT-ACTIONSORT` deletes the sort this rule performs, and this is where
    /// that deletion becomes observable rather than incidental.
    #[test]
    fn ordered_actions_ignore_input_order(spec in arb_graph_spec()) {
        let (first, _) = spec.orders();
        let graph = materialize(&spec, first)?;
        let actions: Vec<(&str, _)> = graph
            .actions
            .iter()
            .map(|(id, action)| (id.as_str(), action))
            .collect();
        let rotated = rotated(&actions, 3);

        prop_assert!(
            action_ids(&actions) != action_ids(&rotated),
            "the constructed rotation must actually perturb the input order",
        );
        prop_assert_eq!(
            action_ids(&ordered_actions(actions.iter().copied())),
            action_ids(&ordered_actions(rotated.iter().copied())),
            "the rule must ignore the order its actions arrive in",
        );
    }

    /// One fixed graph value, inserted two ways, emits the same bundle.
    ///
    /// The comparison covers the `build.ninja` text and every dyndep sidecar's
    /// path, content, and position. Position is compared because the bundle
    /// publishes its sidecars in generation order, so two runs holding the same
    /// sidecars differently ordered have still diverged.
    #[test]
    fn one_graph_emits_one_bundle_under_two_insertion_orders(spec in arb_graph_spec()) {
        let (first, second) = spec.orders();
        let first_bundle = bundle(&spec, first)?;
        let second_bundle = bundle(&spec, second)?;

        prop_assert_eq!(
            first_bundle.build_file(),
            second_bundle.build_file(),
            "the build file must depend on the graph, not on insertion order",
        );
        prop_assert_eq!(
            sidecars(&first_bundle),
            sidecars(&second_bundle),
            "dyndep sidecars must be published in a content-determined order",
        );
    }
}

/// One graph's emitted bundle, reporting a refused graph as a failure.
///
/// Reported rather than unwrapped so a generator defect arrives as a shrinkable
/// failure carrying the specification's shape. The generated domain avoids the
/// reserved namespaces and the unsupported characters, so a refusal is a
/// generator defect rather than an expected outcome.
///
/// # Errors
///
/// Returns a test failure when the emitter refuses the graph.
fn bundle(
    spec: &GraphSpec,
    order: &[usize],
) -> Result<crate::ninja_gen::GeneratedNinja, TestCaseError> {
    let graph = materialize(spec, order)?;
    generate_bundle(&graph)
        .map_err(|error| TestCaseError::fail(format!("emit one insertion order: {error}")))
}

/// Every sidecar's path and content, in bundle order.
fn sidecars(bundle: &crate::ninja_gen::GeneratedNinja) -> Vec<(&str, &str)> {
    bundle
        .dyndep_files()
        .iter()
        .map(|file| (file.relative_path().as_str(), file.content()))
        .collect()
}

/// An edge sequence's sort keys, which are unique within a generated graph.
///
/// The pool hands out disjoint, pre-numbered slices, so no two edges of one
/// specification share a first output. Comparing keys therefore compares the
/// arrangement as a reader of the emitted file would see it.
fn edge_keys(edges: &[&BuildEdge]) -> Vec<String> {
    edges
        .iter()
        .map(|edge| path_key(&edge.explicit_outputs))
        .collect()
}

/// An action sequence's identifiers.
fn action_ids<'a>(actions: &[(&'a str, &crate::ir::Action)]) -> Vec<&'a str> {
    actions.iter().map(|(id, _)| *id).collect()
}

/// Rotate a sequence by `offset`, wrapping, mirroring the production rule.
///
/// The production rotation in `strategy::graph` is typed on pool indices and is
/// private to that module, so this restates it for the two sequence types the
/// core arm perturbs. A one-element sequence has no alternative arrangement and
/// is returned unchanged; no property here assumes otherwise.
///
/// The offset is clamped to the last index rather than reduced modulo the
/// length: `rotate_left` requires a count no greater than the length, and the
/// workspace denies `integer_division_remainder_used`.
fn rotated<T: Copy>(items: &[T], offset: usize) -> Vec<T> {
    let Some(last_index) = items.len().checked_sub(1) else {
        return Vec::new();
    };
    let mut rotated = items.to_vec();
    if last_index > 0 {
        rotated.rotate_left(offset.min(last_index).max(1));
    }
    rotated
}

/// What a run's cases looked like, tallied so the arm cannot go quiet.
///
/// The two divergence counts are the end-to-end arm's honesty: a run whose two
/// materializations always iterated identically asserted equality over
/// identical inputs and is evidence for nothing. The class counts are the
/// obligation's structural non-vacuity — multi-output edges, phony edges,
/// dyndep staging, and defaults must coexist in one graph, and every
/// `always`/`restat` combination must occur.
#[derive(Clone, Copy, Debug, Default)]
struct RunShape {
    /// Cases whose two materializations iterated their edges differently.
    diverged: usize,
    /// Cases whose two materializations iterated their actions differently.
    actions_diverged: usize,
    /// Cases reaching the multi-output and phony intersection.
    multi_output_and_phony: usize,
    /// Cases reaching multi-output and dyndep staging together.
    multi_output_and_staged: usize,
    /// Cases declaring at least one default target.
    with_defaults: usize,
    /// Cases reaching the full class intersection.
    intersections: usize,
    /// Union of the `always`/`restat` combinations observed, as a mask.
    always_restat: u8,
}

impl RunShape {
    /// Fold one case's specification and its two materializations in.
    fn extended(self, spec: &GraphSpec, diverged: bool, actions_diverged: bool) -> Self {
        let classes: Classification = spec.classification();
        let multi_output = classes.multi_output_edges > 0;
        Self {
            diverged: self.diverged + usize::from(diverged),
            actions_diverged: self.actions_diverged + usize::from(actions_diverged),
            multi_output_and_phony: self.multi_output_and_phony
                + usize::from(multi_output && classes.phony_edges > 0),
            multi_output_and_staged: self.multi_output_and_staged
                + usize::from(multi_output && classes.dyndep_staged_edges > 0),
            with_defaults: self.with_defaults + usize::from(classes.default_targets > 0),
            intersections: self.intersections + usize::from(classes.reaches_class_intersection()),
            always_restat: self.always_restat | classes.always_restat_seen,
        }
    }
}

/// Whether two materializations of one specification iterate their orders apart.
///
/// Raw iteration order is what is compared, not the sorted result: once the
/// rule has sorted, two arrangements are indistinguishable by construction, so
/// comparing the rule's output here would make the question circular.
///
/// # Errors
///
/// Returns a test failure when the specification cannot be materialized.
fn orders_diverge(spec: &GraphSpec) -> Result<(bool, bool), TestCaseError> {
    let (first, second) = spec.orders();
    let first_graph = materialize(spec, first)?;
    let second_graph = materialize(spec, second)?;
    let edges_differ = edge_lists_differ(
        &first_graph.edges().cloned().collect::<Vec<_>>(),
        &second_graph.edges().cloned().collect::<Vec<_>>(),
    );
    let first_actions: Vec<&str> = first_graph.actions.keys().map(String::as_str).collect();
    let second_actions: Vec<&str> = second_graph.actions.keys().map(String::as_str).collect();
    Ok((edges_differ, first_actions != second_actions))
}

/// The run diverges, and reaches every class the obligation names.
///
/// The generated domain's lower bounds are a sampling threshold rather than a
/// correctness bound, so a run may legitimately contain cases whose
/// materializations coincide. What it must not do is contain *only* those, and
/// what it must reach is the class intersection plus all four `always`/`restat`
/// combinations.
#[test]
fn the_run_diverges_and_reaches_every_required_class() {
    let tally: Cell<RunShape> = Cell::new(RunShape::default());
    coverage_runner(128)
        .run(&arb_graph_spec(), |spec| {
            let (diverged, actions_diverged) = orders_diverge(&spec)?;
            tally.set(tally.get().extended(&spec, diverged, actions_diverged));
            Ok(())
        })
        .expect("the graph strategy should be generatable");

    let observed = tally.into_inner();
    macro_rules! covered {
        ($field:ident, $why:expr) => {
            assert!(observed.$field > 0, "{}, reached: {observed:?}", $why);
        };
    }
    covered!(
        diverged,
        "no case's two materializations iterated their edges differently, so \
         the end-to-end arm compared one arrangement to itself and is \
         inconclusive rather than passed"
    );
    covered!(
        actions_diverged,
        "no case's two materializations iterated their actions differently"
    );
    covered!(
        multi_output_and_phony,
        "no case combined multi-output edges with phony ones"
    );
    covered!(
        multi_output_and_staged,
        "no case combined multi-output edges with dyndep staging"
    );
    covered!(with_defaults, "no case declared a default target");
    covered!(
        intersections,
        "no case reached multi-output and phony and dyndep-staged and defaults \
         together, which is the intersection OBL-ORDER names"
    );
    assert_eq!(
        observed.always_restat, ALWAYS_RESTAT_ALL,
        "the run must exercise all four always/restat combinations; reached: {observed:?}"
    );
}

/// An edge's sort key is the sorted output set, not its first output.
///
/// The difference only shows on edges whose output sets interleave: here both
/// edges begin at `out/a`, so ordering by first output leaves them in arrival
/// order, while ordering by set puts `out/a out/m` before `out/a out/z`. This
/// is the directed witness for the `path_key` call inside the rule, which
/// `MUT-EDGESORT` deletes along with the sort.
#[test]
fn the_edge_rule_keys_on_the_output_set() {
    use crate::ir::DependencyOrder;
    use camino::Utf8PathBuf;

    let edge = |outputs: [&str; 2]| BuildEdge {
        action_id: "act-0000".to_owned(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        dependency_order: DependencyOrder::Parallel,
        explicit_outputs: outputs.map(Utf8PathBuf::from).to_vec(),
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    };
    let later = edge(["out/a", "out/z"]);
    let earlier = edge(["out/a", "out/m"]);
    assert!(
        earlier.explicit_outputs.first() == later.explicit_outputs.first(),
        "the witness needs two edges whose first outputs agree",
    );
    assert!(
        path_key(&earlier.explicit_outputs) < path_key(&later.explicit_outputs),
        "the constructed pair must order differently by set than by first path",
    );
    assert_eq!(
        edge_keys(&ordered_edges([&later, &earlier].into_iter())),
        [
            path_key(&earlier.explicit_outputs),
            path_key(&later.explicit_outputs),
        ],
        "the edge rule must order by the sorted output set",
    );
}
