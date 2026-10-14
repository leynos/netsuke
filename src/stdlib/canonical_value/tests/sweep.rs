//! Generated sweep: the oracle checked at scale, and the strategy's coverage.
//!
//! Every case here draws its inputs from a strategy rather than writing them
//! out, which is the one thing the sibling `oracle` battery cannot do: a named
//! witness proves the production comparison runs on *that* input, and this
//! battery proves it runs on inputs nobody chose. That is a stronger claim for
//! correctness and a weaker one for any individual shape, so the two are kept
//! apart deliberately.
//!
//! The correctness case and the coverage case use different runners on purpose.
//! A law is worth checking against fresh inputs, so it keeps `proptest`'s random
//! seed; a claim about the strategy wants a frozen sample, because otherwise an
//! unlucky seed could report a shape as missing from a generator that still
//! emits it — a defect in the test rather than in the generator. `properties`
//! states the same split for the same reason.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Result, ensure};
use minijinja::value::Value;
use proptest::prelude::*;

use super::oracle::{check_against_the_oracle, oracle_indices};
use super::properties::{coverage_runner, generated_values, law_runner};
use super::*;

/// The repeat structure of a generated input, so a tally can assert the sweep
/// reached it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Shape {
    /// No two elements share a canonical key.
    Distinct,
    /// Every element shares one canonical key.
    Uniform,
    /// Some key repeats and some other key appears too.
    Mixed,
}

/// Classify a sequence by how its keys repeat, refusing inadmissible values.
fn shape_of(values: &[Value]) -> Result<Shape> {
    let keys = keys_of(values)?;
    if keys.len() <= 1 || !has_repeat(&keys) {
        return Ok(Shape::Distinct);
    }
    if keys.iter().collect::<HashSet<&CanonicalKey>>().len() == 1 {
        return Ok(Shape::Uniform);
    }
    Ok(Shape::Mixed)
}

/// Return the canonical keys of `values`, refusing inadmissible ones.
fn keys_of(values: &[Value]) -> Result<Vec<CanonicalKey>> {
    values
        .iter()
        .map(|value| canonical_key(value).context("generated values are admitted"))
        .collect()
}

/// Report whether a slice of canonical keys holds a repeat.
fn has_repeat(keys: &[CanonicalKey]) -> bool {
    let distinct: HashSet<&CanonicalKey> = keys.iter().collect();
    distinct.len() != keys.len()
}

/// Generate a sequence in which one drawn element is duplicated.
///
/// The repeat is built rather than filtered for: an element admitted once is
/// admitted again by cloning it, so a duplicate is guaranteed whatever the
/// generator's value pool does. A sweep that filtered for repeats instead would
/// thin out exactly as the value pool widened.
fn sequences_with_a_repeated_element() -> BoxedStrategy<Vec<Value>> {
    prop::collection::vec(generated_values(), 1..7)
        .prop_flat_map(|values| {
            let positions = 0..values.len();
            let insertions = 0..=values.len();
            (positions, insertions, Just(values))
        })
        .prop_map(|(source, insert_at, mut values)| {
            let twin = values.get(source).cloned().unwrap_or_default();
            values.insert(insert_at, twin);
            values
        })
        .boxed()
}

/// Generate either an ordinary generated sequence or one carrying a repeat.
///
/// The mixture is what makes the shape tally meaningful: ordinary sequences
/// supply the all-distinct shape, duplicated ones guarantee a repeat, and the
/// tally asserts both were reached on the frozen sample.
fn sweep_sequences() -> BoxedStrategy<Vec<Value>> {
    prop_oneof![
        prop::collection::vec(generated_values(), 0..8),
        sequences_with_a_repeated_element(),
    ]
    .boxed()
}

/// Check every generated sequence against the independent oracle.
///
/// Retained values must be the caller's own values at exactly the oracle's
/// indices, in the oracle's order. That is stricter than the law case in
/// `properties`, which counts distinct keys and so accepts an implementation
/// that keeps a later representative or reorders the output. The runner keeps
/// proptest's free seed, so this case makes no claim about which shapes the
/// sample contains: asserting that here would let an unlucky seed fail a
/// correct implementation, which is the sampling-dependent defect the sibling
/// coverage case below exists to avoid.
#[test]
fn deduplication_agrees_with_the_independent_oracle() -> Result<()> {
    let mut runner = law_runner();
    runner
        .run(&sweep_sequences(), |values| {
            check_against_the_oracle(&values)
        })
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    Ok(())
}

/// Assert the sweep's strategy reaches every shape EP-V3 names, and that it
/// reaches an input whose key actually repeats.
///
/// This is the non-vacuity guard for the sweep above. The repeat it asserts is
/// the oracle's skip branch, so the tally names it explicitly rather than
/// inferring it from the shapes: a sweep of all-distinct inputs would satisfy a
/// shape assertion while leaving that branch untested.
#[test]
fn the_sweep_reaches_every_shape_ep_v3_names() -> Result<()> {
    let counts: RefCell<BTreeMap<Shape, usize>> = RefCell::new(BTreeMap::new());
    let dropped = AtomicUsize::new(0);
    let mut runner = coverage_runner();
    runner
        .run(&sweep_sequences(), |values| {
            let shape =
                shape_of(&values).map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
            *counts.borrow_mut().entry(shape).or_insert(0) += 1;
            let expected = oracle_indices(&values)
                .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
            if expected.len() < values.len() {
                dropped.fetch_add(1, Ordering::Relaxed);
            }
            Ok(())
        })
        .map_err(|error| anyhow::anyhow!("{error}"))?;

    let tally = counts.into_inner();
    for shape in [Shape::Distinct, Shape::Uniform, Shape::Mixed] {
        ensure!(
            tally.get(&shape).copied().unwrap_or(0) > 0,
            "shape {shape:?} was never generated in this sweep: {tally:?}",
        );
    }
    ensure!(
        dropped.load(Ordering::Relaxed) > 0,
        "the sweep must contain an input with a repeated key, or the oracle's \
         duplicate branch went untested",
    );
    Ok(())
}
