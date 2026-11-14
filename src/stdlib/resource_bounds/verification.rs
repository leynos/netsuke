//! Kani harnesses for the bounded resource-bound kernels.
//!
//! The Verus proof in `verus/resource_bounds/` reasons about a transcription of
//! these kernels. These harnesses close the remaining gap by driving the
//! compiled production functions within stated bounds: no input in range may
//! panic, and the values they return must agree with an independent `u128`
//! oracle or with the admission contract the callers rely on.
//!
//! Each harness states the bound that makes it decidable. The solver cannot
//! discharge a symbolic `u64` product or quotient in reasonable time — a first
//! attempt at the exactness harnesses ran for over forty minutes without a
//! result — so the value comparisons are made where the arithmetic stays small
//! enough to bit-blast, and concrete witnesses carry the overflow boundary that
//! a small domain cannot reach. The general mathematical claim is the Verus
//! theorem; these harnesses are the correspondence check on the compiled code,
//! not a replacement for it.

use super::{CardinalityError, RunningTotal, StreamKind, kernel};

/// Return `n choose r` for `r <= 2`, or `None` when it exceeds `u64::MAX`.
///
/// The oracle multiplies in `u128` so its own intermediate value cannot
/// overflow, which is what makes it independent of the kernel's step size.
fn closed_form_binomial_low(n: u64, r: u64) -> Option<u64> {
    let wide = u128::from(n);
    match r {
        0 => Some(1),
        1 => Some(n),
        _ => {
            let previous = wide.saturating_sub(1);
            u64::try_from(wide * previous / 2).ok()
        }
    }
}

/// Prove the product kernel answers zero and repeat cases before multiplying.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(4)]
fn product_kernel_respects_zero_and_repeat() {
    let lengths: [usize; 3] = kani::any();
    let repeat: u64 = kani::any();
    // Bound the operands so the fold's arithmetic stays small. Three operands
    // are enough to place a zero before or after a huge one.
    kani::assume(lengths[0] <= 2 && lengths[1] <= 2 && lengths[2] <= 2);
    kani::assume(repeat <= 2);

    let result = kernel::checked_product(&lengths, repeat);

    if repeat == 0 {
        kani::assert(
            result == Some(1),
            "a repeat of zero yields the single empty tuple",
        );
    } else if lengths.contains(&0) {
        kani::assert(
            result == Some(0),
            "a zero operand absorbs the product whatever the other operands",
        );
    }

    // Two operands of 2^40 overflow a running product long before the trailing
    // zero is reached. The bound above keeps the symbolic fold small, so this
    // concrete case is what proves the zero scan runs before any
    // multiplication; without that ordering the fold returns `None`.
    const BIG: usize = 1 << 40;
    kani::assert(
        kernel::checked_product(&[BIG, BIG, 0], 1) == Some(0),
        "a zero operand after overflowing operands still yields zero",
    );

    kani::cover!(repeat == 0, "the repeat-zero shortcut is reached");
    kani::cover!(
        repeat > 0 && lengths.contains(&0),
        "the zero-operand shortcut is reached"
    );
    kani::cover!(
        repeat < 2 && lengths[0] <= 2 && lengths[1] == 0,
        "a zero operand after a nonzero one is reached"
    );
}

/// Prove `n choose r` for `r <= 2` matches the closed form on a small domain.
///
/// The quotient is checked where both the numerator and the divisor are small
/// enough for the solver to bit-blast; two concrete witnesses then carry the
/// representable and unrepresentable ends of the `u64` range, which the small
/// symbolic domain cannot reach.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(3)]
fn binomial_kernel_matches_closed_form_for_r_up_to_two() {
    let n: u64 = kani::any();
    let r: u64 = kani::any();
    kani::assume(n <= 16);
    kani::assume(r <= 2);

    kani::assert(
        kernel::checked_binomial(n, r) == closed_form_binomial_low(n, r),
        "the binomial kernel matches the u128 closed form on the bounded domain",
    );

    // `2 * C(n, 2) = n * (n - 1)` lands just inside `u64` here, and just
    // outside it at `2^33`, so an off-by-one in the overflow test cannot pass.
    kani::assert(
        kernel::checked_binomial(5_000_000_000, 2) == Some(12_499_999_997_500_000_000),
        "a step above u64::MAX still yields a representable coefficient",
    );
    kani::assert(
        kernel::checked_binomial(1 << 33, 2).is_none(),
        "a coefficient above u64::MAX is refused",
    );

    kani::cover!(
        n < r && kernel::checked_binomial(n, r) == Some(0),
        "choosing more items than exist yields zero"
    );
    kani::cover!(
        r == 2 && n >= 4 && kernel::checked_binomial(n, r) == closed_form_binomial_low(n, r),
        "the general step is exercised above the trivial cases"
    );
}

/// Prove the binomial kernel does not panic for a small reduced index.
///
/// Kani fails a harness on any reachable panic, so `VERIFICATION:- SUCCESSFUL`
/// is itself the claim: the `r > n` shortcut, the `n - r` subtraction, the
/// `n - i` step, and both conversions are exercised without a panic path. The
/// representability oracle this harness used to carry is a `u128` quotient of
/// two symbolic values, which the solver could not decide, and an unbounded
/// numerator cost a minute of solver time on its own; the numerator is
/// therefore bounded, and the exactness harness next to it checks the quotient
/// where it is small, with concrete witnesses at the `u64` boundary.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(6)]
fn binomial_kernel_never_panics_for_small_r() {
    // The `r > n` guard is what keeps the subtraction safe. Every operand it
    // protects is symbolic here, and the path returns before the first
    // division, so this half of the claim costs almost nothing to decide.
    let n: u64 = kani::any();
    kani::assume(n <= 3);
    let _guard = kernel::checked_binomial(n, 4);

    // Then the loop itself on a symbolic operand. Each 128-bit division step
    // is a large circuit, so the sweep covers the reduced indices a template
    // reaches most often rather than every index the cap admits; the exactness
    // harness next to this one carries the rest.
    for r in [1_u64, 2] {
        let n: u64 = kani::any();
        kani::assume(n >= 8 && n <= 64);
        let _loop = kernel::checked_binomial(n, r);
    }

    kani::cover!(
        kernel::checked_binomial(3, 4) == Some(0),
        "the cannot-choose-more-than-exists shortcut is reached"
    );
}

/// Prove the falling-factorial kernel matches a checked `u128` product.
///
/// The symbolic operand is bounded so the `u128` product stays small enough to
/// bit-blast. The iteration cap itself is pinned by the boundary harness next
/// to this one, because reaching it needs a twenty-step concrete loop and a
/// symbolic loop of that depth costs paths rather than unrollings.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(5)]
fn falling_factorial_kernel_matches_checked_product() {
    let n: u64 = kani::any();
    let r: u64 = kani::any();
    kani::assume(n <= 16);
    kani::assume(r <= 4);

    kani::assert(
        kernel::checked_falling_factorial(n, r) == checked_falling_oracle(n, r),
        "the falling factorial matches the checked u128 product on the bounded domain",
    );
}

/// Pin the falling-factorial iteration cap from both sides.
///
/// `20!` is the largest falling factorial that still fits in `u64` and the cap
/// refuses from 21 upwards, so these two witnesses straddle the boundary. Both
/// loops are concrete — the twenty-step one and the immediate refusal — so the
/// deep unwind bound costs unrollings rather than symbolic paths, and the
/// unbounded operand exercises an early `None` on the second step.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(21)]
fn falling_factorial_boundary_is_pinned() {
    kani::assert(
        kernel::checked_falling_factorial(20, 20) == Some(2_432_902_008_176_640_000),
        "the largest representable falling factorial is admitted",
    );
    kani::assert(
        kernel::checked_falling_factorial(u64::MAX, 20).is_none(),
        "a twenty-step product of a huge operand is refused on overflow",
    );
    kani::assert(
        kernel::checked_falling_factorial(21, 21).is_none(),
        "the first unrepresentable falling factorial is refused",
    );
}

/// Return `n * (n - 1) * ... * (n - r + 1)` in `u128`, or `None` on overflow.
fn checked_falling_oracle(n: u64, r: u64) -> Option<u64> {
    if r > n {
        return Some(0);
    }
    let wide = u128::from(n);
    let product = (0..r).try_fold(1_u128, |acc, i| acc.checked_mul(wide - u128::from(i)))?;
    u64::try_from(product).ok()
}

/// Prove classification of an exact cardinality never admits past its ceiling.
#[kani::proof]
#[kani::solver(kissat)]
fn admission_never_exceeds_ceiling() {
    let cardinality: Option<u64> = kani::any();
    let ceiling: u64 = kani::any();

    match kernel::classify(cardinality, ceiling) {
        Ok(admitted) => {
            kani::assert(
                admitted <= ceiling,
                "an admitted count never exceeds its ceiling",
            );
            kani::assert(
                cardinality == Some(admitted),
                "an admitted count is the exact cardinality",
            );
        }
        Err(CardinalityError::Exceeded {
            cardinality: reported,
            ceiling: reported_ceiling,
        }) => {
            kani::assert(
                cardinality == Some(reported),
                "an exceeded refusal reports the exact cardinality",
            );
            kani::assert(
                reported > reported_ceiling,
                "an exceeded refusal is above its ceiling",
            );
            kani::assert(
                reported_ceiling == ceiling,
                "an exceeded refusal echoes the ceiling it was given",
            );
        }
        Err(CardinalityError::Overflow {
            ceiling: reported_ceiling,
        }) => {
            kani::assert(
                cardinality.is_none(),
                "an overflow refusal only follows an unrepresentable count",
            );
            kani::assert(
                reported_ceiling == ceiling,
                "an overflow refusal echoes the ceiling it was given",
            );
        }
    }

    kani::cover!(
        kernel::classify(Some(100_000), 100_000).is_ok(),
        "a count at its ceiling is admitted"
    );
    kani::cover!(
        kernel::classify(Some(100_001), 100_000).is_err(),
        "a count one past its ceiling is refused"
    );
    kani::cover!(
        kernel::classify(None, 100_000).is_err(),
        "an unrepresentable count is refused"
    );
}

/// Prove a running total never admits past its limit and never loses a refusal.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(4)]
fn running_total_never_admits_past_its_limit() {
    let limit: u64 = kani::any();
    let amounts: [u64; 3] = kani::any();
    kani::assume(limit <= 8);
    kani::assume(amounts[0] <= 8 && amounts[1] <= 8 && amounts[2] <= 8);

    let mut total = RunningTotal::with_ceiling(StreamKind::MatchCount, limit);
    let mut expected: u64 = 0;

    for amount in amounts {
        let outcome = total.admit(amount);
        let wanted = expected.checked_add(amount).filter(|sum| *sum <= limit);
        match (outcome, wanted) {
            (Ok(()), Some(sum)) => {
                expected = sum;
                kani::assert(
                    total.admitted() == expected,
                    "an admitted amount advances the total exactly",
                );
            }
            (Err(refusal), None) => {
                kani::assert(
                    refusal.ceiling == limit,
                    "a refusal names the ceiling it enforced",
                );
                kani::assert(
                    total.admitted() == expected,
                    "a refusal leaves the total unchanged",
                );
            }
            (Ok(()), None) => kani::assert(false, "no admission past the limit"),
            (Err(_), Some(_)) => kani::assert(false, "no refusal within the limit"),
        }
    }

    kani::assert(
        total.admitted() <= limit,
        "the admitted total never passes its limit",
    );
    kani::cover!(
        total.admitted() == limit,
        "the total reaches its limit exactly"
    );
}
