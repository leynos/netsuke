//! Depth, input-length, and stream boundaries.
//!
//! Each of these bounds is a single comparison or one checked addition, so the
//! cases here are the whole obligation rather than a sample of it: the table
//! walks both sides of every ceiling, and the generated cases cover the same
//! rule over a range no table would enumerate.
//!
//! The stream cases count pulls rather than retained items, because the
//! promise is that a collector stops as soon as the ceiling is passed. That
//! only holds if the collector checks while it grows, so the count is the
//! observable that distinguishes the two implementations.

use anyhow::{Result, anyhow};
use googletest::prelude::*;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use rstest::rstest;

use super::*;

/// Cases the generated depth walk runs.
const DEPTH_CASES: u32 = 64;

/// Cases the running-total property runs.
const TOTAL_CASES: u32 = 256;

/// The largest artificial limit the running-total property uses.
const SMALL_LIMIT: u64 = 1_000;

/// The largest single amount the running-total property adds.
const SMALL_AMOUNT: u64 = 400;

/// Walk `descents` levels from the root, reporting the outcome.
fn walk(descents: u32) -> Result<NestingDepth, DepthExceeded> {
    (0..descents).try_fold(NestingDepth::root(), |depth, _| depth.descend())
}

/// Depths either side of the nesting ceiling.
#[rstest]
#[case::root(0, true)]
#[case::first_level(1, true)]
#[case::one_before_the_ceiling(127, true)]
#[case::at_the_ceiling(128, true)]
#[case::one_past_the_ceiling(129, false)]
fn descents_are_admitted_exactly_up_to_the_ceiling(
    #[case] descents: u32,
    #[case] is_admitted: bool,
) {
    let walked = walk(descents);
    assert_eq!(
        walked.is_ok(),
        is_admitted,
        "admission must agree with the ceiling"
    );
    match walked {
        Ok(depth) => assert_eq!(
            depth.level(),
            descents,
            "the level must report the descents made"
        ),
        Err(refused) => {
            assert_eq!(
                refused,
                DepthExceeded {
                    depth: descents,
                    ceiling: NESTING_DEPTH_CEILING,
                },
                "a refused descent must name the depth it attempted"
            );
        }
    }
}

/// Property: a walk of `d` levels succeeds exactly when `d` is within the
/// ceiling.
#[test]
fn descents_match_the_ceiling_for_generated_depths() -> Result<()> {
    let mut runner = TestRunner::new(Config {
        cases: DEPTH_CASES,
        ..Config::default()
    });
    runner
        .run(&(0_u32..=200), |descents| {
            let walked = walk(descents);
            prop_assert_eq!(
                walked.is_ok(),
                descents <= NESTING_DEPTH_CEILING,
                "admission must match the ceiling"
            );
            Ok(())
        })
        .map_err(|error| anyhow!("{error}"))?;
    Ok(())
}

/// A refused amount leaves the running total unchanged.
#[test]
fn running_total_refuses_the_amount_that_passes_the_ceiling() {
    let mut total = RunningTotal::with_ceiling(StreamKind::MatchCount, 10);
    total.admit(7).expect("seven of ten is admitted");
    assert_eq!(total.admitted(), 7, "the admitted amounts must accumulate");

    let refused = total.admit(4).expect_err("eleven of ten is refused");
    assert_that!(
        refused,
        eq(StreamLimitExceeded {
            kind: StreamKind::MatchCount,
            ceiling: 10,
        })
    );
    assert_eq!(
        total.admitted(),
        7,
        "a refused amount must not change the total"
    );

    total.admit(3).expect("exactly the ceiling is admitted");
    assert_eq!(total.admitted(), 10, "the total must reach the ceiling");
}

/// A checked-add overflow is a refusal, not a wrapped total.
#[test]
fn running_total_refuses_an_addition_that_overflows() {
    let mut total = RunningTotal::with_ceiling(StreamKind::OutputBytes, u64::MAX);
    total
        .admit(u64::MAX - 1)
        .expect("within the artificial ceiling");
    total.admit_one().expect("exactly the artificial ceiling");

    let refused = total
        .admit_one()
        .expect_err("one past u64::MAX must be refused");
    assert_that!(
        refused,
        eq(StreamLimitExceeded {
            kind: StreamKind::OutputBytes,
            ceiling: u64::MAX,
        })
    );
    assert_eq!(
        total.admitted(),
        u64::MAX,
        "a refused addition must leave the total at its last value"
    );
}

/// One total carried across three documents trips on the third.
///
/// This is the shape the stream bound exists for: a per-document total would
/// never reach the ceiling, and only a total shared across the whole stream
/// refuses the third document.
#[test]
fn one_running_total_is_shared_across_documents() {
    let mut total = RunningTotal::with_ceiling(StreamKind::MatchCount, 10);
    let documents = [3_u64, 4, 4];
    let mut refused_at = None;
    for (index, amount) in documents.iter().enumerate() {
        if total.admit(*amount).is_err() {
            refused_at = Some(index);
            break;
        }
    }
    assert_eq!(
        refused_at,
        Some(2),
        "the third document must be the one refused"
    );
    assert_eq!(
        total.admitted(),
        7,
        "the refused document must contribute nothing"
    );
}

/// Property: the running total agrees with a `u64` checked-add oracle.
#[test]
fn running_total_matches_the_checked_oracle() -> Result<()> {
    let mut runner = TestRunner::new(Config {
        cases: TOTAL_CASES,
        ..Config::default()
    });
    runner
        .run(
            &(
                0_u64..=SMALL_LIMIT,
                prop::collection::vec(0_u64..=SMALL_AMOUNT, 0..8),
            ),
            |(limit, amounts)| {
                let mut total = RunningTotal::with_ceiling(StreamKind::OutputBytes, limit);
                let mut expected = 0_u64;
                for amount in amounts {
                    match expected.checked_add(amount).filter(|sum| *sum <= limit) {
                        Some(next) => {
                            total
                                .admit(amount)
                                .map_err(|error| TestCaseError::fail(error.to_string()))?;
                            expected = next;
                        }
                        None => prop_assert!(
                            total.admit(amount).is_err(),
                            "the oracle refused this amount"
                        ),
                    }
                    prop_assert_eq!(
                        total.admitted(),
                        expected,
                        "the running total must equal the oracle"
                    );
                }
                Ok(())
            },
        )
        .map_err(|error| anyhow!("{error}"))?;
    Ok(())
}

/// An unbounded match stream is refused one item past the ceiling.
#[test]
fn match_collection_stops_one_item_past_the_ceiling() {
    let mut stream = CountingStream::new(std::iter::repeat(1_u64));
    let refused =
        collect_matches(stream.by_ref()).expect_err("an unbounded stream exceeds the ceiling");
    assert_that!(
        refused,
        eq(StreamLimitExceeded {
            kind: StreamKind::MatchCount,
            ceiling: MATCH_COUNT_CEILING,
        })
    );
    assert_eq!(
        stream.pulls(),
        100_001,
        "the collector must stop after one item past the ceiling"
    );
}

/// A stream holding exactly the ceiling is collected whole.
#[test]
fn match_collection_accepts_exactly_the_ceiling() {
    let mut stream = CountingStream::new(0..MATCH_COUNT_CEILING);
    let collected = collect_matches(stream.by_ref()).expect("exactly the ceiling is admitted");
    assert_eq!(
        collected.len(),
        usize::try_from(MATCH_COUNT_CEILING).expect("the ceiling fits a usize"),
        "every item up to the ceiling must be retained"
    );
    assert_eq!(
        collected.last().copied(),
        Some(MATCH_COUNT_CEILING - 1),
        "the last item must be the one before the ceiling"
    );
}

/// A per-item failure is returned unchanged, and nothing after it is pulled.
#[test]
fn try_collect_matches_stops_at_the_first_item_error() {
    let mut stream = CountingStream::new((0..10).map(|position| {
        if position == 4 {
            Err(StreamLimitExceeded {
                kind: StreamKind::MatchCount,
                ceiling: MATCH_COUNT_CEILING,
            })
        } else {
            Ok(position)
        }
    }));
    let refused = try_collect_matches(stream.by_ref()).expect_err("the fifth item fails");
    assert_that!(
        refused,
        eq(StreamLimitExceeded {
            kind: StreamKind::MatchCount,
            ceiling: MATCH_COUNT_CEILING,
        })
    );
    assert_eq!(
        stream.pulls(),
        5,
        "nothing after the failing item may be pulled"
    );
}

/// Input lengths either side of the byte ceiling.
#[rstest]
#[case::empty(0, true)]
#[case::one_byte_under(8_388_607, true)]
#[case::at_the_ceiling(8_388_608, true)]
#[case::one_byte_past(8_388_609, false)]
#[case::largest_representable(u64::MAX, false)]
fn input_lengths_are_admitted_exactly_up_to_the_ceiling(
    #[case] length: u64,
    #[case] is_admitted: bool,
) {
    let outcome = super::super::limits::admit_length(length);
    assert_eq!(
        outcome.is_ok(),
        is_admitted,
        "admission must agree with the ceiling"
    );
    if let Err(refused) = outcome {
        assert_that!(
            refused,
            eq(InputLengthExceeded {
                length,
                ceiling: INPUT_LENGTH_CEILING,
            })
        );
    }
}

/// The ceiling counts bytes, not characters.
///
/// The fixture is many fewer characters than the ceiling and many more bytes, so
/// a length taken from `chars().count()` would admit it. That is the whole
/// point of the case: the parser must scan bytes, so the bound must be stated in
/// bytes.
#[test]
fn input_length_counts_bytes_rather_than_characters() {
    let text = "é".repeat(4_194_305);
    assert_eq!(
        text.chars().count(),
        4_194_305,
        "the fixture must hold fewer characters than the ceiling"
    );
    assert_eq!(
        text.len(),
        8_388_610,
        "the fixture must hold more bytes than the ceiling"
    );
    let refused = admit_input(&text).expect_err("the byte length exceeds the ceiling");
    assert_that!(
        refused,
        eq(InputLengthExceeded {
            length: 8_388_610,
            ceiling: INPUT_LENGTH_CEILING,
        })
    );
}
