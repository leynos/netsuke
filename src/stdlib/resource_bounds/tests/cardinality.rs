//! Named arithmetic witnesses for exactness, overflow, and absorption.
//!
//! Each case pins one integer derived by hand — a product exactly at
//! `u64::MAX`, the first central binomial that no longer fits, the exponent at
//! which a power of two overflows, the longest arrangement that still fits —
//! and cross-checks it against the independent oracle. A kernel that is wrong
//! in one branch changes one of these numbers, so the cases fail individually
//! rather than as a mass, which is what makes them useful as seeded-fault
//! controls.
//!
//! The witnesses are grouped by kernel because that is how a seeded fault is
//! introduced and reverted: a fault in the product fold must fail the product
//! witnesses and leave the others untouched.

use rstest::rstest;

use super::kernel;
use super::oracle;
use super::*;

/// Half of `u64::MAX`, written out because integer division is denied.
const HALF_OF_U64_MAX: u64 = 9_223_372_036_854_775_807;

/// Witnesses for the product fold and its power step.
///
/// The two halves of the request are exercised together: `repeat == 0` and a
/// zero operand decide before any multiplication, `repeat == 63` is the last
/// power of two that fits, and `repeat == 64` is the first that cannot.
#[rstest]
#[case::exactly_max(&[4_294_967_295, 4_294_967_297], 1, Some(u64::MAX))]
#[case::past_max(&[4_294_967_296, 4_294_967_296], 1, None)]
#[case::zero_absorbs_huge_operands(&[1_099_511_627_776, 1_099_511_627_776, 0], 1, Some(0))]
#[case::one_raised_a_million_times(&[1], 1_000_000, Some(1))]
#[case::power_fits(&[2], 63, Some(9_223_372_036_854_775_808))]
#[case::power_overflows(&[2], 64, None)]
#[case::no_operands_and_no_repetition(&[], 0, Some(1))]
#[case::zero_operand_with_no_repetition(&[0], 0, Some(1))]
fn product_witnesses(
    #[case] lengths: &[usize],
    #[case] repeat: u64,
    #[case] expected: Option<u64>,
) {
    let request = CardinalityRequest::Product { lengths, repeat };
    assert_eq!(
        request.cardinality(),
        expected,
        "the kernel must reproduce the named product"
    );
    assert_eq!(
        oracle::product(lengths, repeat),
        expected,
        "the oracle must reproduce the named product"
    );
}

/// Witnesses for the binomial coefficient, cross-checked against Pascal.
///
/// `C(67, 33)` is the largest central coefficient that fits in a `u64`, and
/// `C(68, 34)` is the first that does not, so the pair pins the iteration cap
/// from both sides.
#[rstest]
#[case::largest_central_fit(67, 33, Some(14_226_520_737_620_288_370))]
#[case::next_central_overflows(68, 34, None)]
#[case::nothing_from_five(5, 6, Some(0))]
#[case::one_from_five(5, 1, Some(5))]
#[case::everything_from_five(5, 5, Some(1))]
fn combination_witnesses(#[case] length: usize, #[case] r: u64, #[case] expected: Option<u64>) {
    let request = CardinalityRequest::Combinations { length, r };
    assert_eq!(
        request.cardinality(),
        expected,
        "the kernel must reproduce the named coefficient"
    );
    assert_eq!(
        oracle::binomial(
            u64::try_from(length).expect("the witness length fits a u64"),
            r
        ),
        expected,
        "the Pascal oracle must reproduce the named coefficient"
    );
}

/// The symmetric reduction is what makes a near-complete selection cheap.
///
/// `combinations(100000, 99999)` is `100000`, and it is answered by reducing
/// the loop to a single step. Without the reduction the kernel would iterate
/// `99999` times, which the cap refuses, so the value alone distinguishes the
/// two implementations.
#[test]
fn near_complete_selection_uses_the_symmetric_reduction() {
    let request = CardinalityRequest::Combinations {
        length: 100_000,
        r: 99_999,
    };
    assert_eq!(
        request.cardinality(),
        Some(100_000),
        "the reduction must make the near-complete selection exact"
    );
}

/// Witnesses for the falling factorial and its cap.
#[rstest]
#[case::twenty_fits(20, 20, Some(2_432_902_008_176_640_000))]
#[case::twenty_one_overflows(21, 21, None)]
#[case::more_than_available(5, 6, Some(0))]
#[case::one_position(5, 1, Some(5))]
#[case::no_positions(5, 0, Some(1))]
fn permutation_witnesses(#[case] length: usize, #[case] r: u64, #[case] expected: Option<u64>) {
    let request = CardinalityRequest::Permutations { length, r };
    assert_eq!(
        request.cardinality(),
        expected,
        "the kernel must reproduce the named arrangement"
    );
    assert_eq!(
        oracle::permutations(
            u64::try_from(length).expect("the witness length fits a u64"),
            r
        ),
        expected,
        "the oracle must reproduce the named arrangement"
    );
}

/// The binomial cap refuses before the loop, whatever the coefficient is.
///
/// A reduced iteration count of 34 or more cannot produce a `u64`, so the cap
/// check answers `None` without walking the loop at all. The case is here
/// rather than only in Kani because it also pins the cap's boundary: 33 steps
/// still run and 34 do not.
#[test]
fn binomial_cap_stops_symbolically_large_inputs() {
    assert_eq!(
        kernel::checked_binomial(u64::MAX, HALF_OF_U64_MAX),
        None,
        "a central coefficient with a reduced count above the cap is unrepresentable"
    );
    assert_eq!(
        kernel::checked_binomial(u64::MAX, 34),
        None,
        "the cap boundary is 34, not 33"
    );
    assert_eq!(
        kernel::checked_binomial(u64::MAX, 33),
        None,
        "33 steps run, and the coefficient itself still overflows"
    );
}

/// The falling-factorial cap refuses before the loop for the same reason.
#[test]
fn falling_factorial_cap_stops_symbolically_large_inputs() {
    assert_eq!(
        kernel::checked_falling_factorial(1_000_000_000_000_000_000, 1_000_000_000_000_000_000),
        None,
        "an arrangement longer than the cap is unrepresentable"
    );
    assert_eq!(
        kernel::checked_falling_factorial(1_000_000_000_000_000_000, 20),
        None,
        "20 steps run, and the product itself still overflows"
    );
}
