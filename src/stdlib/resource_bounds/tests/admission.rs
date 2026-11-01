//! Admission boundaries: what is admitted, and what a refusal costs.
//!
//! The boundary table is the RFC 0015 §5.8 contract stated as numbers: each row
//! is one side of a ceiling, and the permutation rows exist to show that the
//! lower permutation ceiling is selected by the request kind rather than by the
//! caller. The `classify` property covers the same rule over every ceiling a
//! `u64` can express, which the table cannot enumerate.
//!
//! The remaining cases make the allocation argument checkable. A refusal is
//! `Copy` and carries only integers and enumerations, and a request that no
//! machine could materialize is refused promptly — so nothing was built before
//! the check. That is a structural argument backed by observation, not a
//! measurement: a counting allocator would need `unsafe` or a new dependency.

use anyhow::{Result, anyhow};
use googletest::prelude::*;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use rstest::rstest;

use super::kernel;
use super::*;

/// Cases the `classify` property runs.
const CLASSIFY_CASES: u32 = 512;

/// Admission rows, one per side of a ceiling.
///
/// Each row names the request that reaches that side. `product` and
/// `combinations` share [`OUTPUT_TUPLE_CEILING`]; `permutations` uses
/// [`PERMUTATION_TUPLE_CEILING`], which is the row that fails if the ceiling is
/// selected from the cardinality instead of the request kind.
#[rstest]
#[case::product_at_ceiling(
    CardinalityRequest::Product { lengths: &[100_000], repeat: 1 },
    Ok(100_000),
)]
#[case::product_one_past(
    CardinalityRequest::Product { lengths: &[100_001], repeat: 1 },
    Err(CardinalityError::Exceeded { cardinality: 100_001, ceiling: OUTPUT_TUPLE_CEILING }),
)]
#[case::product_two_operands(
    CardinalityRequest::Product { lengths: &[316, 317], repeat: 1 },
    Err(CardinalityError::Exceeded { cardinality: 100_172, ceiling: OUTPUT_TUPLE_CEILING }),
)]
#[case::combinations_at_ceiling(
    CardinalityRequest::Combinations { length: 100_000, r: 99_999 },
    Ok(100_000),
)]
#[case::combinations_one_past(
    CardinalityRequest::Combinations { length: 100_001, r: 1 },
    Err(CardinalityError::Exceeded { cardinality: 100_001, ceiling: OUTPUT_TUPLE_CEILING }),
)]
#[case::permutations_at_ceiling(
    CardinalityRequest::Permutations { length: 10_000, r: 1 },
    Ok(10_000),
)]
#[case::permutations_one_past(
    CardinalityRequest::Permutations { length: 10_001, r: 1 },
    Err(CardinalityError::Exceeded { cardinality: 10_001, ceiling: PERMUTATION_TUPLE_CEILING }),
)]
#[case::permutations_eight_of_eight(
    CardinalityRequest::Permutations { length: 8, r: 8 },
    Err(CardinalityError::Exceeded { cardinality: 40_320, ceiling: PERMUTATION_TUPLE_CEILING }),
)]
fn admission_reports_the_exact_verdict(
    #[case] request: CardinalityRequest<'_>,
    #[case] expected: Result<usize, CardinalityError>,
) {
    let outcome = request.admit().map(super::AdmittedCount::get);
    assert_that!(outcome, eq(expected));
}

/// Property: `classify` admits exactly the values at or below the ceiling.
///
/// The property is stated over every `Option<u64>` and every ceiling, which the
/// table cannot enumerate. It pins the precedence rule too: `None` is refused
/// as overflow even though it is certainly above the ceiling.
#[test]
fn classify_admits_exactly_the_values_within_the_ceiling() -> Result<()> {
    let mut runner = TestRunner::new(Config {
        cases: CLASSIFY_CASES,
        ..Config::default()
    });
    runner
        .run(
            &(any::<Option<u64>>(), any::<u64>()),
            |(cardinality, ceiling)| {
                match kernel::classify(cardinality, ceiling) {
                    Ok(value) => {
                        prop_assert_eq!(
                            Some(value),
                            cardinality,
                            "an admission must return the cardinality it was given"
                        );
                        prop_assert!(
                            value <= ceiling,
                            "an admitted value must not exceed its ceiling"
                        );
                    }
                    Err(CardinalityError::Exceeded {
                        cardinality: reported,
                        ceiling: reported_ceiling,
                    }) => {
                        prop_assert!(
                            cardinality == Some(reported),
                            "an exceeded refusal must name the exact cardinality"
                        );
                        prop_assert!(
                            reported > ceiling,
                            "an exceeded refusal must name a value above the ceiling"
                        );
                        prop_assert_eq!(
                            reported_ceiling,
                            ceiling,
                            "the refusal must name the ceiling"
                        );
                    }
                    Err(CardinalityError::Overflow {
                        ceiling: reported_ceiling,
                    }) => {
                        prop_assert!(
                            cardinality.is_none(),
                            "overflow is reserved for an unrepresentable cardinality"
                        );
                        prop_assert_eq!(
                            reported_ceiling,
                            ceiling,
                            "the refusal must name the ceiling"
                        );
                    }
                }
                Ok(())
            },
        )
        .map_err(|error| anyhow!("{error}"))?;
    Ok(())
}

/// The cardinality refusal is `Copy` and holds no owned data.
///
/// The assertion is a compile-time one on purpose: it fails the build, not a
/// run, the moment a refusal gains a `String`, a `Vec`, or a Fluent key. That
/// is the property the allocation argument rests on, and it is the only way to
/// state it without a counting allocator.
#[test]
fn cardinality_refusals_are_copy_and_hold_only_values() {
    /// Require `T` to be `Copy`.
    fn assert_copy<T: Copy>() {}

    assert_copy::<CardinalityError>();
}

/// A request no machine could materialize is refused promptly.
///
/// The permutation count is `usize::MAX` items taken `u32::MAX` at a time,
/// which is far past the falling-factorial cap on every supported target, so
/// the refusal is decided from the lengths alone. An implementation that built
/// output first would abort rather than return, which is why this case is worth
/// keeping even though it measures nothing.
#[test]
fn an_unmaterializable_request_is_refused() {
    let request = CardinalityRequest::Permutations {
        length: usize::MAX,
        r: u64::from(u32::MAX),
    };
    let refused = request
        .admit()
        .expect_err("no machine can materialize this many arrangements");
    assert_that!(
        refused,
        eq(CardinalityError::Overflow {
            ceiling: PERMUTATION_TUPLE_CEILING
        })
    );
}
