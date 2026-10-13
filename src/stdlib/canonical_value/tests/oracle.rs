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
//! duplicates, and order-distinct sequences — are stated here on authored
//! inputs whose expected indices are written out. Each named witness drives
//! [`check_against_the_oracle`] as well as the index expectation, so the
//! production comparison runs on a written-down input with no sampling in the
//! way, which is what the sibling `sweep` battery cannot offer: its inputs come
//! from a strategy, so its coverage is a separate claim about that strategy.

use std::collections::HashSet;

use anyhow::{Result, ensure};
use minijinja::value::Value;
use proptest::prelude::*;
use rstest::rstest;

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

/// Assert the runtime retains the caller's own values at the oracle's indices.
///
/// This is the whole claim under test: the same length, the same order, and
/// the identical original value at each position. Pointer identity is checked
/// with [`std::ptr::eq`], so a reconstruction of an equal value fails here even
/// though it would pass a key comparison.
pub(super) fn check_against_the_oracle(input: &[Value]) -> std::result::Result<(), TestCaseError> {
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
pub(super) fn oracle_indices(input: &[Value]) -> Result<Vec<usize>> {
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

/// Assert the oracle's indices cover each distinct key once, and that the
/// runtime agrees with them on this same input.
///
/// The two claims are checked together at every named witness, so a witness
/// cannot assert an index expectation the production function happens not to
/// meet: the comparison is the point of the witness, and the coverage check is
/// what stops the index expectation from being satisfied by a degenerate
/// answer.
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
    check_against_the_oracle(input).map_err(|error| {
        anyhow::anyhow!("the runtime must match the oracle on this witness: {error}")
    })
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
