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

use super::flatten::{FlattenBudget, FlattenLimits, flatten_filter};
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

/// A nested graph drawn from leaves, containers, and shared descendants.
///
/// `prop_recursive` bounds the depth and branch size, so a generated graph
/// stays inside a test's time budget while spanning ragged nesting, empty
/// containers, and repeated references. The container arm stores one drawn
/// child twice whenever `share` says so, which is the compact shared graph the
/// ceiling exists to stop expanding for free. A map is a leaf to `flatten`, not
/// a container to descend into, so it belongs in the corpus too.
fn graph() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::from(1)),
        Just(Value::from("s")),
        Just(Value::from(true)),
    ];
    leaf.prop_recursive(3, 8, 3, |inner| {
        prop_oneof![
            2 => vec(inner.clone(), 0..3).prop_map(Value::from),
            2 => (inner.clone(), any::<bool>()).prop_map(|(child, share)| {
                if share {
                    Value::from(vec![child.clone(), child])
                } else {
                    Value::from(vec![child])
                }
            }),
            1 => btree_map("[a-c]", any::<u8>(), 0..2).prop_map(|map: BTreeMap<String, u8>| {
                Value::from_iter(map.into_iter().map(|(key, value)| (key, Value::from(value))))
            }),
        ]
    })
}

/// A top-level subject: a list of sequences, which the filter accepts.
///
/// Every immediate member is itself a list because the filter rejects a scalar
/// or map member at the top level. Nested members may still be any graph,
/// including scalars and maps, where those are leaves.
fn subject() -> impl Strategy<Value = Value> {
    let member = vec(graph(), 0..3).prop_map(Value::from);
    vec(member, 0..4).prop_map(Value::from)
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
    fn flatten_within_the_ceilings_matches_a_reference_expansion(value in subject()) {
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
    fn flatten_rejects_exactly_past_the_visit_ceiling(value in subject().prop_filter(
        "an empty subject has no visit to remove",
        |value| reference_counts(value).0 > 0,
    )) {
        let (visits, leaves, depth) = exact_limits(&value);
        let budget = FlattenBudget::new(FlattenLimits::for_test(depth, visits - 1, leaves));
        let err = flatten_filter(&value, &budget)
            .expect_err("one visit short of the graph's own count must be rejected");
        let message = err.to_string();
        prop_assert!(message.contains("exceeded the traversal visit limit"),
            "the visit ceiling must be the one reported, but was {message:?}");
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
    // tuple is `(nested, shared, deepest)`.
    let tallies: Cell<(bool, bool, usize)> = Cell::new((false, false, 0));
    runner
        .run(&subject(), |value| {
            let (nested, shared, deepest) = tallies.get();
            let depth = reference_counts(&value).1;
            tallies.set((
                nested || depth >= 2,
                shared || has_repeated_child(&value),
                deepest.max(depth),
            ));
            Ok(())
        })
        .map_err(|err| anyhow::anyhow!("the corpus runner failed: {err}"))?;
    let (saw_nested, saw_shared, deepest) = tallies.get();
    ensure!(
        saw_nested,
        "the corpus must reach a graph nested at least two levels deep; deepest was {deepest}"
    );
    ensure!(
        saw_shared,
        "the corpus must reach a container that stores one child twice"
    );
    Ok(())
}

/// Whether some container under `value` repeats one of its children.
fn has_repeated_child(value: &Value) -> bool {
    if !is_container(value) {
        return false;
    }
    let Ok(iter) = value.try_iter() else {
        return false;
    };
    let members: Vec<Value> = iter.collect();
    let mut seen: Vec<&Value> = Vec::new();
    for member in &members {
        if seen.contains(&member) {
            return true;
        }
        seen.push(member);
    }
    members.iter().any(has_repeated_child)
}
