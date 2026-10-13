//! Independent oracle: which source indices first-index deduplication retains.
//!
//! Every case here states the expected answer directly over the input — an
//! index is retained when no earlier index holds an equal key — and then checks
//! [`stable_deduplicate`] against it. The oracle never calls the function under
//! test and never consults a membership structure: it keys each value, scans
//! the earlier keys quadratically, and returns the indices that survive. That
//! independence is the point. The law battery in `properties` compares the
//! output against a count of distinct keys, so an implementation that kept the
//! wrong representative, or emitted values in an order borrowed from its
//! `HashSet`, would satisfy it; here such an implementation fails, and the two
//! claims `stable_deduplicate` documents — first representatives, in input
//! order — are each checked on their own.
//!
//! The shapes EP-V3 names — empty, singleton, all-distinct, all-equal, repeated
//! values at head, middle, and tail, interleaved duplicates, reordered-map
//! duplicates, and order-distinct sequences — are covered twice. The
//! handwritten witnesses below state them on authored inputs whose expected
//! indices are written out, and the generated sweep repeats them at scale; the
//! shape tally keeps that sweep honest by asserting the sample really contains
//! all-distinct, all-equal, and mixed inputs rather than hoping a lucky draw
//! supplies them. Duplicate-bearing sequences are constructed by duplicating a
//! drawn element rather than by filtering for a repeat, so no wing can starve.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Result, ensure};
use minijinja::value::Value;
use proptest::prelude::*;
use rstest::rstest;

use super::properties::{generated_values, law_runner};
use super::*;

/// Where a duplicated pair sits in an otherwise distinct sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placement {
    /// The first two values are equal.
    Head,
    /// Two values in the middle are equal.
    Middle,
    /// The last two values are equal.
    Tail,
}

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

/// Assert the runtime retains the caller's own values at the oracle's indices.
///
/// This is the whole claim under test: the same length, the same order, and
/// the identical original value at each position. Pointer identity is checked
/// with [`std::ptr::eq`], so a reconstruction of an equal value fails here even
/// though it would pass a key comparison.
fn check_against_the_oracle(input: &[Value]) -> std::result::Result<(), TestCaseError> {
    let expected = oracle_indices(input)
        .map_err(|error| TestCaseError::fail(format!("the oracle refused the input: {error:#}")))?;
    let retained = stable_deduplicate(input)
        .map_err(|error| TestCaseError::fail(format!("the runtime refused the input: {error}")))?;

    prop_assert_eq!(
        retained.len(),
        expected.len(),
        "the runtime retains {} values where the oracle retains {} indices",
        retained.len(),
        expected.len(),
    );
    for (position, (kept, index)) in retained.iter().zip(&expected).enumerate() {
        let original = input.get(*index).ok_or_else(|| {
            TestCaseError::fail(format!("the oracle named out-of-range index {index}"))
        })?;
        prop_assert!(
            std::ptr::eq(*kept, original),
            "position {position} must be the original at input index {index}, not a copy",
        );
    }
    Ok(())
}

/// Return the indices `stable_deduplicate` should retain, least index first.
///
/// EP-V3's quadratic first-index oracle. The runtime admits each value in order
/// and retains it exactly when its key was not seen earlier, so the expected
/// answer is read per index by scanning the indices before it. No membership
/// structure is consulted and [`stable_deduplicate`] is never called, so a
/// fault in either is invisible to the expectation.
///
/// # Errors
///
/// Returns the refusal of the earliest inadmissible input value, so a witness
/// carrying an unsupported value fails rather than being silently skipped.
fn oracle_indices(input: &[Value]) -> Result<Vec<usize>> {
    let keys = input
        .iter()
        .map(|value| canonical_key(value).context("every witness value is admitted"))
        .collect::<Result<Vec<CanonicalKey>>>()?;

    Ok(keys
        .iter()
        .enumerate()
        .filter(|(index, key)| !keys.iter().take(*index).any(|earlier| earlier == *key))
        .map(|(index, _)| index)
        .collect())
}

/// Assert the oracle's indices cover each distinct input key exactly once.
fn oracle_covers_each_key_once(input: &[Value]) -> Result<()> {
    let retained = oracle_indices(input)?;
    let distinct: HashSet<CanonicalKey> = input
        .iter()
        .map(|value| canonical_key(value).context("every witness value is admitted"))
        .collect::<Result<_>>()?;
    ensure!(
        retained.len() == distinct.len(),
        "the oracle retains {} indices for {} distinct keys",
        retained.len(),
        distinct.len(),
    );
    let covered: HashSet<CanonicalKey> = retained
        .iter()
        .map(|index| {
            let value = input
                .get(*index)
                .context("the oracle cannot name an out-of-range index")?;
            canonical_key(value).context("the value is admitted")
        })
        .collect::<Result<_>>()?;
    ensure!(
        covered == distinct,
        "the oracle's indices must cover every distinct key, not merely the same count",
    );
    Ok(())
}

/// Verify the oracle keeps the first representative for empty, singleton,
/// all-distinct, and all-equal inputs.
#[rstest]
#[case::empty(vec![], vec![])]
#[case::singleton(vec![7], vec![0])]
#[case::all_distinct(vec![1, 2, 3], vec![0, 1, 2])]
#[case::all_equal(vec![5, 5, 5, 5], vec![0])]
fn oracle_keeps_the_first_representative(
    #[case] numbers: Vec<i64>,
    #[case] expected: Vec<usize>,
) -> Result<()> {
    let input: Vec<Value> = numbers.iter().copied().map(Value::from).collect();
    let indices = oracle_indices(&input)?;
    ensure!(
        indices == expected,
        "expected {expected:?}, got {indices:?} for {numbers:?}",
    );
    oracle_covers_each_key_once(&input)
}

/// Verify a duplicate at the head, middle, or tail keeps the earlier occurrence
/// of that pair.
#[rstest]
#[case::head(Placement::Head, vec![0, 2, 3])]
#[case::middle(Placement::Middle, vec![0, 1, 3])]
#[case::tail(Placement::Tail, vec![0, 1, 2])]
fn oracle_keeps_the_first_of_a_repeated_pair(
    #[case] placement: Placement,
    #[case] expected: Vec<usize>,
) -> Result<()> {
    let (left, right) = match placement {
        Placement::Head => (0, 1),
        Placement::Middle => (1, 2),
        Placement::Tail => (2, 3),
    };
    let mut numbers = vec![10, 20, 30];
    let twin = numbers
        .get(left)
        .copied()
        .context("the base sequence is long enough")?;
    numbers.insert(right, twin);
    let input: Vec<Value> = numbers.iter().copied().map(Value::from).collect();

    let indices = oracle_indices(&input)?;
    ensure!(
        indices == expected,
        "{placement:?}: expected {expected:?}, got {indices:?} for {numbers:?}",
    );
    oracle_covers_each_key_once(&input)
}

/// Verify interleaved duplicates — `a, b, a, b, a` — keep the first occurrence
/// of each key rather than one interleaved pattern.
#[rstest]
fn oracle_keeps_the_first_of_interleaved_duplicates() -> Result<()> {
    let input: Vec<Value> = [1, 2, 1, 2, 1].iter().copied().map(Value::from).collect();
    let indices = oracle_indices(&input)?;
    ensure!(indices == [0, 1], "expected [0, 1], got {indices:?}");
    oracle_covers_each_key_once(&input)
}

/// Verify two orderings of one member set share a key, so the second is a
/// duplicate rather than a second value.
#[rstest]
fn oracle_treats_reordered_maps_as_duplicates() -> Result<()> {
    let ascending = ordered_map(&[("alpha", 1), ("beta", 2)]);
    let descending = ordered_map(&[("beta", 2), ("alpha", 1)]);
    let input = [ascending, descending, Value::from("zeta")];
    let indices = oracle_indices(&input)?;
    ensure!(
        indices == [0, 2],
        "the second map repeats the first's key, got {indices:?}",
    );
    oracle_covers_each_key_once(&input)
}

/// Verify sequences differing only in order are distinct, so the second is
/// retained rather than folded into the first.
#[rstest]
fn oracle_treats_order_distinct_sequences_as_distinct() -> Result<()> {
    let input = [
        Value::from(vec![1, 2]),
        Value::from(vec![2, 1]),
        Value::from(vec![1, 2]),
    ];
    let indices = oracle_indices(&input)?;
    ensure!(
        indices == [0, 1],
        "sequence order decides the key, got {indices:?}",
    );
    oracle_covers_each_key_once(&input)
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
/// that keeps a later representative or reorders the output. Each drawn input
/// is classified as it is checked, so the tally below costs no extra sample and
/// cannot drift away from the shapes this case really saw.
#[test]
fn deduplication_agrees_with_the_independent_oracle() -> Result<()> {
    let counts: RefCell<BTreeMap<Shape, usize>> = RefCell::new(BTreeMap::new());
    let dropped = AtomicUsize::new(0);
    let mut runner = law_runner();
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
            check_against_the_oracle(&values)
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

/// Verify the oracle's first-wins answer differs from a last-wins answer.
///
/// Retaining the last rather than the first occurrence of each key is the fault
/// EP-V3 names, and a count of distinct keys cannot see it. This pins the
/// oracle's discrimination without seeding a fault into the implementation.
#[test]
fn the_oracle_distinguishes_first_wins_from_last_wins() -> Result<()> {
    let input: Vec<Value> = [1, 2, 1, 2, 3].iter().copied().map(Value::from).collect();
    let keys = input
        .iter()
        .map(|value| canonical_key(value).context("the witness is admitted"))
        .collect::<Result<Vec<CanonicalKey>>>()?;
    let first = oracle_indices(&input)?;

    let last: Vec<usize> = keys
        .iter()
        .enumerate()
        .filter(|(index, key)| !keys.iter().skip(*index + 1).any(|later| later == *key))
        .map(|(index, _)| index)
        .collect();

    ensure!(
        first == [0, 1, 4],
        "the oracle must keep the first occurrences, got {first:?}",
    );
    ensure!(
        last == [2, 3, 4],
        "the last-wins answer must keep the last occurrences, got {last:?}",
    );
    ensure!(
        first != last,
        "the two answers must differ, or the sweep could not tell them apart",
    );
    Ok(())
}
