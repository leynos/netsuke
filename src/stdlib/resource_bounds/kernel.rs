//! The checked arithmetic kernel behind the public admission API.
//!
//! Every function here is pure and total, and the public types call them
//! rather than duplicating their arithmetic. That is deliberate: this module
//! is the unit the Verus model mirrors and the Kani harnesses drive, so it is
//! kept free of `usize` lengths, refusal construction, and `Display`.
//!
//! # Why the iteration caps are structural
//!
//! [`checked_binomial`] and [`checked_falling_factorial`] refuse before
//! entering their loops once the reduced iteration count is known to overflow
//! every `u64`. Bounded work then follows from the code rather than from
//! arithmetic growth, and the thresholds are proved rather than tuned: see
//! lemmas L8 and L9 in
//! `docs/execplans/6-1-3-shared-bounded-materialization-helper.md`.
//!
//! # Why overflow is `Option`
//!
//! `None` means "greater than `u64::MAX`", not "failed". Every caller turns it
//! into [`CardinalityError::Overflow`], which is a refusal rather than an
//! error condition, so the kernel stays free of the refusal vocabulary.

use super::cardinality::CardinalityError;

/// Reduced binomial iteration count at which the coefficient exceeds `u64`.
///
/// `binom(67, 33)` is the largest central coefficient that still fits, and
/// `binom(n, k)` is at least `binom(68, 34)` for every `34 <= k <= n / 2`, so
/// no coefficient with a reduced `k` of 34 or more is representable.
const BINOMIAL_ITERATION_CAP: u64 = 34;

/// Falling-factorial count at which the product exceeds `u64::MAX`.
///
/// `20!` fits and `21!` does not, and every longer product is at least `21!`.
const FALLING_FACTORIAL_ITERATION_CAP: u64 = 21;

/// Largest exponent a base of at least two can take without overflowing.
///
/// `2^63` fits and `2^64` does not, so any exponent of 64 or more exceeds
/// `u64::MAX` whatever the base.
const POWER_EXPONENT_CAP: u64 = 64;

/// Return the exact tuple count of a `product` request, or `None` on overflow.
///
/// `repeat == 0` yields the single empty tuple before any operand is examined,
/// matching `itertools.product`. Otherwise a zero operand absorbs the whole
/// product, which is why the zero scan runs before any multiplication: an
/// empty operand placed after two huge ones must report zero, not overflow.
pub(super) fn checked_product(lengths: &[usize], repeat: u64) -> Option<u64> {
    if repeat == 0 {
        return Some(1);
    }
    if lengths.contains(&0) {
        return Some(0);
    }
    let base = lengths.iter().try_fold(1_u64, |acc, length| {
        let factor = u64::try_from(*length).ok()?;
        acc.checked_mul(factor)
    })?;
    checked_power(base, repeat)
}

/// Raise `base` to `repeat`, refusing a result above `u64::MAX`.
///
/// A base of zero or one is answered directly. That is not an optimisation
/// detail: a manifest may ask for a million repetitions of a one-element
/// operand, and the shortcut keeps the work bounded by the request's shape
/// rather than by its `repeat` value.
fn checked_power(base: u64, repeat: u64) -> Option<u64> {
    if base <= 1 {
        return Some(base);
    }
    if repeat >= POWER_EXPONENT_CAP {
        return None;
    }
    let exponent = u32::try_from(repeat).ok()?;
    base.checked_pow(exponent)
}

/// Return the exact binomial coefficient `n choose r`, or `None` on overflow.
///
/// The symmetric reduction `k = min(r, n - r)` keeps `combinations(100000,
/// 99999)` cheap: its answer is 100000, but the unreduced loop would run 99999
/// times. The running value is carried in `u128` because a single step can
/// exceed `u64` even when the coefficient it computes still fits; the step is
/// exact, so the division leaves no remainder.
pub(super) fn checked_binomial(n: u64, r: u64) -> Option<u64> {
    if r > n {
        return Some(0);
    }
    let k = r.min(n - r);
    if k >= BINOMIAL_ITERATION_CAP {
        return None;
    }
    let mut coefficient: u128 = 1;
    for i in 0..k {
        let scaled = coefficient.checked_mul(u128::from(n - i))?;
        // The divisor is `i + 1`, which is never zero; `checked_div` states
        // that without a panic path, and L5 proves the quotient is exact.
        coefficient = scaled.checked_div(u128::from(i + 1))?;
        if coefficient > u128::from(u64::MAX) {
            return None;
        }
    }
    u64::try_from(coefficient).ok()
}

/// Return the exact falling factorial `n * (n - 1) * … * (n - r + 1)`.
///
/// `r > n` cannot choose more items than exist and yields zero, matching the
/// combinadic definition rather than an error. The product is accumulated with
/// `checked_mul`, so the first overflowing prefix proves the whole result
/// exceeds `u64::MAX` because every remaining factor is at least one.
pub(super) fn checked_falling_factorial(n: u64, r: u64) -> Option<u64> {
    if r > n {
        return Some(0);
    }
    if r >= FALLING_FACTORIAL_ITERATION_CAP {
        return None;
    }
    let mut product = 1_u64;
    for i in 0..r {
        product = product.checked_mul(n - i)?;
    }
    Some(product)
}

/// Return the new running total when `amount` keeps it within `limit`.
///
/// A checked addition that overflows is also a refusal: the true total is then
/// certainly above the ceiling, and the only alternative would be to wrap or
/// saturate, both of which would misreport how much was admitted.
pub(super) fn admit_total(limit: u64, total: u64, amount: u64) -> Option<u64> {
    total.checked_add(amount).filter(|sum| *sum <= limit)
}

/// Classify an exact cardinality against the ceiling for its request kind.
///
/// Overflow takes precedence: an unrepresentable cardinality is refused as
/// [`CardinalityError::Overflow`] even though it is certainly above the
/// ceiling too. That keeps "the count is too large to state" distinct from
/// "the count is known and too large", which RFC 0015 §5.8 requires because
/// the two map to different diagnostics.
pub(super) const fn classify(
    cardinality: Option<u64>,
    ceiling: u64,
) -> Result<u64, CardinalityError> {
    match cardinality {
        None => Err(CardinalityError::Overflow { ceiling }),
        Some(value) if value > ceiling => Err(CardinalityError::Exceeded {
            cardinality: value,
            ceiling,
        }),
        Some(value) => Ok(value),
    }
}
