//! Property tests for the `flatten` traversal and its shared budget.
//!
//! The exact-threshold table in the sibling `tests` module is the complete
//! small contract: it proves which ceiling fires, and one past it, for each of
//! depth, visits and leaves. What a table of hand-picked shapes cannot show is
//! that the claims hold over the whole domain of nested and shared graphs:
//!
//! - an expansion within the ceilings equals an independent depth-first
//!   reference flattening, so the traversal neither drops, duplicates, nor
//!   reorders a leaf;
//! - a graph is rejected exactly when its own visit, leaf, or depth count
//!   exceeds the matching ceiling, so a permissive or over-eager bound is
//!   detected at the boundary of every generated graph;
//! - two calls on one budget consume the sum of their charges, which is the
//!   per-load sharing the per-call design lacked.
//!
//! The corpus is generated, and the model is written separately from the
//! production traversal, so these are statements about the domain rather than
//! restatements of the implementation.

use super::{FlattenBudget, FlattenLimits, flatten_filter};
use anyhow::{Result, ensure};
use minijinja::value::{Value, ValueKind};
use proptest::collection::{btree_map, vec};
use proptest::prelude::*;
use proptest::test_runner::{FileFailurePersistence, TestRunner};
use std::cell::Cell;
use std::collections::BTreeMap;

/// The persisting failure file for this module's generated cases.
///
/// The default `SourceParallel` policy looks for a `lib.rs` or `main.rs`
/// beside the source and gives up inside a library unit-test module, so a
/// recorded seed would be neither written nor replayed. Name the file.
const REGRESSIONS: &str = "tests/stdlib_collections_flatten.proptest-regressions";

/// The configuration for this module's properties.
fn config(cases: u32) -> ProptestConfig {
    ProptestConfig {
        cases,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSIONS))),
        ..ProptestConfig::default()
    }
}

/// A generated graph and whether it repeats a *traversed* child.
///
/// The flag is set by construction, not inferred by comparing values: two
/// equal leaves are indistinguishable by value but are not a shared
/// descendant, and only the construction site knows which one it made.
///
/// A duplicated leaf does not produce the compact expansion the ceiling
/// exists for, so the flag records only a container that stores a container
/// child twice. The corpus test reads it to prove that shape was generated,
/// rather than merely that two equal scalars appeared.
#[derive(Clone, Debug)]
struct Generated {
    /// The graph the filter will traverse.
    value: Value,
    /// Whether some container stores another container as a child twice.
    shared_container: bool,
}

/// A nested graph drawn from leaves, containers, and shared descendants.
///
/// `prop_recursive` bounds the depth and branch size, so a generated graph
/// stays inside a test's time budget while spanning ragged nesting, empty
/// containers, and repeated references. The shared arm stores one drawn child
/// twice whenever `share` says so, which is the compact graph the ceiling
/// exists to stop expanding for free. A map is a leaf to `flatten`, not a
/// container to descend into, so it belongs in the corpus too.
fn graph() -> impl Strategy<Value = Generated> {
    let leaf = prop_oneof![
        Just(Value::from(1)),
        Just(Value::from("s")),
        Just(Value::from(true)),
    ]
    .prop_map(|value| Generated {
        value,
        shared_container: false,
    });
    leaf.prop_recursive(3, 8, 3, |inner| {
        prop_oneof![
            2 => vec(inner.clone(), 0..3).prop_map(|children| {
                let shared_container = children.iter().any(|child| child.shared_container);
                Generated {
                    value: Value::from(
                        children.into_iter().map(|child| child.value).collect::<Vec<_>>(),
                    ),
                    shared_container,
                }
            }),
            2 => (inner.clone(), any::<bool>()).prop_map(|(child, share)| {
                // Only a repeated container is the traversal-expanding shape;
                // repeating a scalar leaf expands nothing.
                let duplicated_container = share && is_container(&child.value);
                let shared_container = child.shared_container || duplicated_container;
                if share {
                    Generated {
                        value: Value::from(vec![child.value.clone(), child.value]),
                        shared_container,
                    }
                } else {
                    Generated {
                        value: Value::from(vec![child.value]),
                        shared_container,
                    }
                }
            }),
            1 => btree_map("[a-c]", any::<u8>(), 0..2).prop_map(|map: BTreeMap<String, u8>| {
                Generated {
                    value: Value::from_iter(
                        map.into_iter().map(|(key, value)| (key, Value::from(value))),
                    ),
                    shared_container: false,
                }
            }),
        ]
    })
}

/// A top-level subject: a list of sequences, which the filter accepts.
///
/// Every immediate member is itself a list because the filter rejects a scalar
/// or map member at the top level. Nested members may still be any graph,
/// including scalars and maps, where those are leaves.
fn subject() -> impl Strategy<Value = Generated> {
    let member = vec(graph(), 0..3).prop_map(|children| {
        let shared_container = children.iter().any(|child| child.shared_container);
        Generated {
            value: Value::from(
                children
                    .into_iter()
                    .map(|child| child.value)
                    .collect::<Vec<_>>(),
            ),
            shared_container,
        }
    });
    vec(member, 0..4).prop_map(|drawn| {
        let shared_container = drawn.iter().any(|entry| entry.shared_container);
        Generated {
            value: Value::from(
                drawn
                    .into_iter()
                    .map(|entry| entry.value)
                    .collect::<Vec<_>>(),
            ),
            shared_container,
        }
    })
}

/// The visit count and greatest depth the filter charges for `value`.
///
/// Every child of a container is one visit, whatever its kind; the container
/// itself was already charged as its parent's child. Immediate members of the
/// subject are depth one, and each container beneath them adds one. Both
/// measures come from the same walk because they are the same traversal's two
/// outputs, computed through the public `try_iter` surface rather than copied
/// from the filter.
fn reference_counts(value: &Value) -> (usize, usize) {
    if !is_container(value) {
        return (0, 0);
    }
    value.try_iter().map_or((0, 0), |children| {
        children.fold((0, 0), |(visits, depth), child| {
            let (child_visits, child_depth) = reference_counts(&child);
            (visits + 1 + child_visits, depth.max(1 + child_depth))
        })
    })
}

/// The leaves an unlimited depth-first traversal would retain, in order.
///
/// A map, string, or scalar is a leaf; only a sequence or iterable is
/// descended into. This is the independent expansion the filter must match.
fn reference_leaves(value: &Value, out: &mut Vec<Value>) {
    if is_container(value) {
        if let Ok(children) = value.try_iter() {
            for child in children {
                reference_leaves(&child, out);
            }
        }
    } else {
        out.push(value.clone());
    }
}

/// Whether `value` is a container the filter descends into.
fn is_container(value: &Value) -> bool {
    matches!(value.kind(), ValueKind::Seq | ValueKind::Iterable)
}

/// The leaves of a successful filter result.
fn leaves_of(value: &Value) -> Result<Vec<Value>> {
    let iter = value
        .try_iter()
        .map_err(|err| anyhow::anyhow!("the flattened result should be iterable: {err}"))?;
    Ok(iter.collect())
}

/// The exact ceilings a graph must fit.
fn exact_limits(value: &Value) -> (usize, usize, usize) {
    let mut leaves = Vec::new();
    reference_leaves(value, &mut leaves);
    let (visits, depth) = reference_counts(value);
    (visits, leaves.len(), depth)
}

proptest! {
    #![proptest_config(config(96))]

    /// A graph exactly at every ceiling flattens to the reference expansion.
    ///
    /// The ceilings equal the graph's own counts, so a bound that was one unit
    /// too tight would reject a graph it must accept. Comparing the output with
    /// the independent depth-first model then catches a traversal that dropped,
    /// duplicated, or reordered a leaf.
    #[test]
    fn flatten_within_the_ceilings_matches_a_reference_expansion(generated in subject()) {
        let value = generated.value;
        let (visits, leaves, depth) = exact_limits(&value);
        let mut expected = Vec::new();
        reference_leaves(&value, &mut expected);

        let budget = FlattenBudget::new(FlattenLimits::for_test(depth, visits, leaves));
        let flattened = flatten_filter(&value, &budget)
            .map_err(|err| TestCaseError::fail(format!("within-ceiling graph was rejected: {err}")))?;
        let observed = leaves_of(&flattened)
            .map_err(|err| TestCaseError::fail(format!("{err:#}")))?;

        prop_assert_eq!(observed, expected,
            "flatten must match the reference depth-first expansion for {:?}", value);
    }

    /// A graph one visit past the ceiling is rejected by the visit ceiling.
    ///
    /// The ceiling is drawn from the graph itself, so every generated graph
    /// probes its own boundary rather than one chosen example. Depth and leaf
    /// ceilings are exact, so neither can mask the visit error.
    #[test]
    fn flatten_rejects_exactly_past_the_visit_ceiling(generated in subject().prop_filter(
        "an empty subject has no visit to remove",
        |generated| reference_counts(&generated.value).0 > 0,
    )) {
        let value = generated.value;
        let (visits, leaves, depth) = exact_limits(&value);
        let budget = FlattenBudget::new(FlattenLimits::for_test(depth, visits - 1, leaves));
        let err = flatten_filter(&value, &budget)
            .expect_err("one visit short of the graph's own count must be rejected");
        let message = err.to_string();
        prop_assert!(message.contains("exceeded the traversal visit limit"),
            "the visit ceiling must be the one reported, but was {message:?}");
    }

    /// A graph one leaf past the ceiling is rejected by the leaf ceiling.
    ///
    /// The visit ceiling is exact and the depth ceiling is raised above the
    /// graph, so only the leaf ceiling can be the one that fires. This is the
    /// counterpart to the visit case: without it the leaf boundary would be
    /// covered by hand-picked examples alone.
    #[test]
    fn flatten_rejects_exactly_past_the_leaf_ceiling(generated in subject().prop_filter(
        "a leaf-free subject has no leaf to remove",
        |generated| exact_limits(&generated.value).1 > 0,
    )) {
        let value = generated.value;
        let (visits, leaves, depth) = exact_limits(&value);
        let budget = FlattenBudget::new(FlattenLimits::for_test(
            depth,
            visits,
            leaves - 1,
        ));
        let err = flatten_filter(&value, &budget)
            .expect_err("one leaf short of the graph's own count must be rejected");
        let message = err.to_string();
        prop_assert!(message.contains("exceeded the retained item limit"),
            "the retained-item ceiling must be the one reported, but was {message:?}");
    }

    /// A graph one level past the ceiling is rejected by the depth ceiling.
    ///
    /// The visit and leaf ceilings are exact and the depth ceiling is one below
    /// the graph, so the depth ceiling is the only one that can fire.
    #[test]
    fn flatten_rejects_exactly_past_the_depth_ceiling(generated in subject().prop_filter(
        "a flat subject has no depth to remove",
        |generated| reference_counts(&generated.value).1 > 0,
    )) {
        let value = generated.value;
        let (visits, leaves, depth) = exact_limits(&value);
        let budget = FlattenBudget::new(FlattenLimits::for_test(
            depth - 1,
            visits,
            leaves,
        ));
        let err = flatten_filter(&value, &budget)
            .expect_err("one level short of the graph's own depth must be rejected");
        let message = err.to_string();
        prop_assert!(message.contains("exceeded the nesting depth limit"),
            "the depth ceiling must be the one reported, but was {message:?}");
    }

    /// Two calls on one budget consume the sum of their charges.
    ///
    /// Each call alone fits the leaf ceiling; together they do not. A
    /// registration that built a fresh budget per call — or per invocation —
    /// would accept both, which is the defect the shared budget exists to
    /// prevent.
    #[test]
    fn two_calls_consume_their_combined_leaves(
        left in vec(Just(1u8), 1..3),
        right in vec(Just(2u8), 1..3),
    ) {
        let total = left.len() + right.len();
        let budget = FlattenBudget::new(FlattenLimits::for_test(8, 100, total - 1));
        let first = Value::from(vec![Value::from(left.into_iter().map(Value::from).collect::<Vec<_>>())]);
        let second = Value::from(vec![Value::from(right.into_iter().map(Value::from).collect::<Vec<_>>())]);

        let first_outcome = flatten_filter(&first, &budget);
        prop_assert!(first_outcome.is_ok(),
            "the first call must fit a ceiling of {}: {:?}",
            total - 1, first_outcome.err());
        let second_outcome = flatten_filter(&second, &budget);
        prop_assert!(second_outcome.is_err(),
            "the second call must exceed the ceiling of {} once both are charged",
            total - 1);
    }
}

/// The corpus reaches nesting and sharing, so the properties are not vacuous.
///
/// Without this the properties could hold over a corpus that only ever drew
/// scalars or flat lists, and say nothing about the graphs the ceilings exist
/// for. It also confirms the shared arm is reachable, so the combined-charge
/// property is exercised rather than assumed.
#[test]
fn the_generated_corpus_reaches_nesting_and_sharing() -> Result<()> {
    let mut runner = TestRunner::new(config(96));
    // `TestRunner::run` takes an `Fn`, so the tallies live in a `Cell`. The
    // tuple is `(nested, shared_container, deepest)`.
    let tallies: Cell<(bool, bool, usize)> = Cell::new((false, false, 0));
    runner
        .run(&subject(), |generated| {
            let (nested, shared_container, deepest) = tallies.get();
            let depth = reference_counts(&generated.value).1;
            tallies.set((
                nested || depth >= 2,
                // The flag is set where the repeated child is built, and only
                // for a repeated container. Comparing values instead would
                // count two equal, independently drawn leaves as sharing, so
                // the corpus could report the expanding shape exercised when
                // only scalars were duplicated.
                shared_container || generated.shared_container,
                deepest.max(depth),
            ));
            Ok(())
        })
        .map_err(|err| anyhow::anyhow!("the corpus runner failed: {err}"))?;
    let (saw_nested, saw_shared_container, deepest) = tallies.get();
    ensure!(
        saw_nested,
        "the corpus must reach a graph nested at least two levels deep; deepest was {deepest}"
    );
    ensure!(
        saw_shared_container,
        "the corpus must reach a container that stores a container child twice"
    );
    Ok(())
}
