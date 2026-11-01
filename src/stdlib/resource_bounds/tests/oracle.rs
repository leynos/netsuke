//! Independent expectations for the cardinality kernels.
//!
//! Nothing here calls the kernel under test. Each function recomputes the
//! mathematical answer by a different route — a bounded `u128` fold, an
//! additive Pascal triangle, or an integer closed form — so agreement between
//! an oracle and its kernel is evidence rather than a tautology.
//!
//! Every oracle reports "above `u64::MAX`" as `None`, matching the kernel's
//! representation relation: `Some(value)` means the exact answer is `value`,
//! and `None` means the exact answer exceeds what a `u64` can hold. An oracle
//! that overflows its own `u128` arithmetic also answers `None`, which is sound
//! because the true answer is then far above `u64::MAX`.

/// The largest `u64` value, written out so the comparison needs no cast.
const MAX_AS_U128: u128 = 18_446_744_073_709_551_615;

/// Longest Pascal row any oracle call builds.
///
/// Row `n` holds `n + 1` coefficients; the largest coefficient of row 130 is
/// `C(130, 65)`, about `9.51e37`, and the intermediate `C(131, 65)` is about
/// `1.89e38`, both below `2^128`. Bounding the row keeps that argument true
/// without a runtime check.
pub(super) const PASCAL_ROW_LIMIT: u64 = 130;

/// Convert one exact oracle value into the kernel's representation.
#[must_use]
pub(super) fn as_kernel_result(value: u128) -> Option<u64> {
    u64::try_from(value).ok()
}

/// Convert a collection length into the width the oracles compute over.
///
/// Every supported target has a `usize` no wider than 64 bits, so the
/// conversion is exact. Stating it once here keeps every battery free of a
/// narrowing cast and of a fallible conversion inside a generated case, where a
/// panic would be reported as a property failure rather than as what it is.
#[must_use]
pub(super) fn width_of(length: usize) -> u64 {
    u64::try_from(length).unwrap_or(u64::MAX)
}

/// Compute the exact product of `lengths` raised to `repeat`, or `None`.
///
/// The operands are folded first and the result is then raised in `u128`, which
/// is the same quantity the kernel computes but by a route that shares no code
/// with it. A zero operand absorbs the whole product because the fold reaches
/// it, and `repeat == 0` gives the single empty tuple because the empty power
/// is one — both matching `itertools.product`.
#[must_use]
pub(super) fn product(lengths: &[usize], repeat: u64) -> Option<u64> {
    let base = lengths.iter().try_fold(1_u128, |product, length| {
        product.checked_mul(u128::try_from(*length).ok()?)
    })?;
    let raised = (0..repeat).try_fold(1_u128, |product, _| product.checked_mul(base))?;
    as_kernel_result(raised)
}

/// Compute `n choose r` from an additive Pascal triangle, or `None`.
///
/// The triangle is built by repeated addition rather than by the multiplicative
/// formula the kernel uses, so the two routes share only the definition. `r > n`
/// yields zero, matching the combinadic convention instead of an error.
#[must_use]
pub(super) fn binomial(n: u64, r: u64) -> Option<u64> {
    if r > n {
        return Some(0);
    }
    let row = pascal_row(n)?;
    let position = usize::try_from(r).ok()?;
    row.get(position).copied().and_then(as_kernel_result)
}

/// Compute `n choose r` for `r` up to two from its closed form, or `None`.
///
/// This route exists for the `u64`-wide `n` cases, where a Pascal row is far
/// too large to build. `n * (n - 1)` is evaluated in `u128`, which is the only
/// place the intermediate exceeds `u64`; the kernel carries the same
/// intermediate, so the two agree on where the answer stops fitting.
#[must_use]
pub(super) fn binomial_up_to_two(n: u64, r: u64) -> Option<u64> {
    if r > n {
        return Some(0);
    }
    let pairs = match r {
        0 => 1_u128,
        1 => u128::from(n),
        2 => u128::from(n)
            .checked_mul(u128::from(n) - 1)
            .and_then(|product| product.checked_div(2))?,
        _ => return None,
    };
    as_kernel_result(pairs)
}

/// Compute the exact falling factorial `n * (n - 1) * … * (n - r + 1)`.
///
/// `r > n` cannot choose more items than exist and yields zero; every other
/// case is a `u128` product, so the oracle and the kernel differ only in the
/// width they carry.
#[must_use]
pub(super) fn permutations(n: u64, r: u64) -> Option<u64> {
    if r > n {
        return Some(0);
    }
    let product = (0..r).try_fold(1_u128, |accumulated, offset| {
        accumulated.checked_mul(u128::from(n - offset))
    })?;
    as_kernel_result(product)
}

/// Build Pascal's row `n`, or `None` when `n` exceeds [`PASCAL_ROW_LIMIT`].
fn pascal_row(n: u64) -> Option<Vec<u128>> {
    if n > PASCAL_ROW_LIMIT {
        return None;
    }
    let row = (0..n).fold(vec![1_u128], |previous, _| {
        std::iter::once(1_u128)
            .chain(previous.windows(2).map(|pair| pair.iter().copied().sum()))
            .chain(std::iter::once(1_u128))
            .collect()
    });
    Some(row)
}

/// Assert that [`MAX_AS_U128`] really is `u64::MAX`.
///
/// The constant is spelled out rather than derived so the comparison in
/// [`as_kernel_result`] needs no cast. This case keeps the literal honest: an
/// edited constant would change the oracle's verdict without any other case
/// noticing.
#[test]
fn the_written_maximum_matches_u64() {
    assert_eq!(
        as_kernel_result(MAX_AS_U128),
        Some(u64::MAX),
        "the written constant must be exactly u64::MAX"
    );
    assert_eq!(
        as_kernel_result(MAX_AS_U128 + 1),
        None,
        "one past the constant must be unrepresentable"
    );
}
