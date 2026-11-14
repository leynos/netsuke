//! Lemmas and transcription for the binomial cardinality.
//!
//! The kernel reduces `r` to `min(r, n - r)`, refuses a reduced index of 34 or
//! more (L8), and otherwise walks the row upwards. The lemmas establish that
//! the walk never decreases (L6) and that a reduced index of 34 already
//! exceeds `u64`.
use vstd::prelude::*;
use vstd::arithmetic::mul::*;
use vstd::arithmetic::div_mod::*;
use crate::model::*;

verus! {

// ------------------------------------------------------ binomial lemmas ----

/// `n choose r` is never negative.
proof fn lemma_binom_nonneg(n: int, r: int)
    requires 0 <= n,
    ensures spec_binom(n, r) >= 0,
    decreases n,
{
    if 0 < r && r < n {
        lemma_binom_nonneg(n - 1, r - 1);
        lemma_binom_nonneg(n - 1, r);
        assert(spec_binom(n, r) == spec_binom(n - 1, r - 1) + spec_binom(n - 1, r)) by {
            reveal_with_fuel(spec_binom, 2);
        }
    } else {
        assert(spec_binom(n, r) >= 0) by {
            reveal_with_fuel(spec_binom, 2);
        }
    }
}

/// `binom(n, 1)` is `n`, the number of ways to choose one item.
proof fn lemma_binom_one(n: int)
    requires n >= 1,
    ensures spec_binom(n, 1) == n,
    decreases n,
{
    if n > 1 {
        lemma_binom_one(n - 1);
        assert(spec_binom(n, 1) == spec_binom(n - 1, 0) + spec_binom(n - 1, 1)) by {
            reveal_with_fuel(spec_binom, 2);
        }
        assert(spec_binom(n - 1, 0) == 1) by {
            reveal_with_fuel(spec_binom, 2);
        }
    } else {
        assert(spec_binom(n, 1) == n) by {
            reveal_with_fuel(spec_binom, 2);
        }
    }
}

/// L4: symmetry.
proof fn lemma_binom_symmetry(n: int, r: int)
    requires 0 <= r <= n,
    ensures spec_binom(n, r) == spec_binom(n, n - r),
    decreases n,
{
    if 0 < r && r < n {
        lemma_binom_symmetry(n - 1, r - 1);
        lemma_binom_symmetry(n - 1, r);
        assert(spec_binom(n, r) == spec_binom(n - 1, r - 1) + spec_binom(n - 1, r)) by {
            reveal_with_fuel(spec_binom, 2);
        }
        assert(spec_binom(n, n - r)
            == spec_binom(n - 1, n - r - 1) + spec_binom(n - 1, n - r)) by {
            reveal_with_fuel(spec_binom, 2);
        }
        assert(spec_binom(n - 1, r) == spec_binom(n - 1, n - 1 - r));
        assert(spec_binom(n - 1, r - 1) == spec_binom(n - 1, n - r));
    } else {
        assert(spec_binom(n, r) == spec_binom(n, n - r)) by {
            reveal_with_fuel(spec_binom, 2);
        }
    }
}

/// L5a: absorption, the bridge from Pascal's rule to the multiplicative step.
proof fn lemma_binom_absorption(n: int, k: int)
    requires 0 <= n, 0 <= k,
    ensures (k + 1) * spec_binom(n + 1, k + 1) == (n + 1) * spec_binom(n, k),
    decreases n,
{
    if n == 0 {
        if k > 0 {
            assert(spec_binom(0, k) == 0) by {
                reveal_with_fuel(spec_binom, 2);
            }
            assert(spec_binom(1, k + 1) == 0) by {
                reveal_with_fuel(spec_binom, 2);
            }
        }
    } else if k == 0 {
        lemma_binom_one(n + 1);
        assert(spec_binom(n, 0) == 1) by {
            reveal_with_fuel(spec_binom, 2);
        }
    } else {
        lemma_binom_absorption(n - 1, k - 1);
        lemma_binom_absorption(n - 1, k);
        lemma_binom_nonneg(n - 1, k - 1);
        lemma_binom_nonneg(n - 1, k);
        let a = spec_binom(n - 1, k - 1);
        let b = spec_binom(n - 1, k);
        let c = spec_binom(n, k);
        let d = spec_binom(n, k + 1);
        assert(c == a + b) by {
            reveal_with_fuel(spec_binom, 2);
        }
        assert(spec_binom(n + 1, k + 1) == c + d) by {
            reveal_with_fuel(spec_binom, 2);
        }
        assert(k * c == n * a);
        assert((k + 1) * d == n * b);
        lemma_mul_is_distributive_add(k + 1, c, d);
        lemma_mul_is_distributive_add(n, a, b);
        lemma_mul_succ_left(k, c);
        lemma_mul_succ_left(n, c);
        assert(n * c == n * a + n * b);
        assert((k + 1) * (c + d) == (k + 1) * c + n * b);
        assert((n + 1) * c == n * a + n * b + c);
        assert((k + 1) * c == k * c + c);
        assert((k + 1) * (c + d) == (n + 1) * c);
    }
}

/// L5: the exact multiplicative step the loop relies on.
proof fn lemma_binom_step(n: int, i: int)
    requires 0 <= i, i + 1 <= n,
    ensures spec_binom(n, i) * (n - i) == spec_binom(n, i + 1) * (i + 1),
{
    if i == 0 {
        lemma_binom_one(n);
        lemma_mul_basics(n);
        lemma_mul_basics(spec_binom(n, 0));
        lemma_mul_basics(spec_binom(n, 1));
        assert(spec_binom(n, 0) == 1) by {
            reveal_with_fuel(spec_binom, 2);
        }
        assert(spec_binom(n, 0) * (n - 0) == spec_binom(n, 1) * (0 + 1));
    } else {
        lemma_binom_absorption(n - 1, i);
        lemma_binom_absorption(n - 1, i - 1);
        lemma_binom_nonneg(n - 1, i - 1);
        lemma_binom_nonneg(n - 1, i);
        let c = spec_binom(n, i);
        let d = spec_binom(n, i + 1);
        let p = spec_binom(n - 1, i - 1);
        let q = spec_binom(n - 1, i);
        assert(c == p + q) by {
            reveal_with_fuel(spec_binom, 2);
        }
        assert((i + 1) * d == n * q);
        assert(i * c == n * p);
        lemma_mul_is_distributive_add(n, p, q);
        lemma_mul_is_distributive_add(i, p, q);
        assert(n * c == n * p + n * q);
        assert(i * c == i * p + i * q);
        assert(n * c == i * c + (i + 1) * d);
        assert(c * (n - i) == c * n - c * i) by {
            lemma_mul_is_distributive_sub(c, n, i);
        }
        assert(c * n == n * c) by {
            lemma_mul_is_commutative(c, n);
        }
        assert(c * i == i * c) by {
            lemma_mul_is_commutative(c, i);
        }
        assert(c * (n - i) == d * (i + 1));
    }
}

/// L6: the coefficient grows up to the middle of the row. The side condition is
/// stated as `i + 1 <= n - i` so that every step stays in linear arithmetic.
proof fn lemma_binom_monotone(n: int, i: int)
    requires 0 <= i, i + 1 <= n - i,
    ensures spec_binom(n, i) <= spec_binom(n, i + 1),
    decreases n,
{
    if i == 0 {
        lemma_binom_one(n);
        assert(spec_binom(n, 0) == 1) by {
            reveal_with_fuel(spec_binom, 2);
        }
    } else if i + 1 == n - i {
        lemma_binom_symmetry(n, i);
    } else {
        lemma_binom_monotone(n - 1, i - 1);
        lemma_binom_monotone(n - 1, i);
        assert(spec_binom(n, i)
            == spec_binom(n - 1, i - 1) + spec_binom(n - 1, i)) by {
            reveal_with_fuel(spec_binom, 2);
        }
        assert(spec_binom(n, i + 1)
            == spec_binom(n - 1, i) + spec_binom(n - 1, i + 1)) by {
            reveal_with_fuel(spec_binom, 2);
        }
    }
}

/// Monotonicity in the row: binom(n, k) <= binom(n + 1, k).
proof fn lemma_binom_row_monotone(n: int, k: int)
    requires 0 <= n, 0 <= k <= n,
    ensures spec_binom(n, k) <= spec_binom(n + 1, k),
{
    lemma_binom_nonneg(n, k - 1);
    assert(spec_binom(n + 1, k) == spec_binom(n, k - 1) + spec_binom(n, k)) by {
        reveal_with_fuel(spec_binom, 2);
    }
}

/// Grow the same column from a shorter row: binom(n, k) <= binom(m, k).
proof fn lemma_binom_row_between(n: int, m: int, k: int)
    requires 34 <= k, k <= n <= m,
    ensures spec_binom(n, k) <= spec_binom(m, k),
    decreases m,
{
    if m > n {
        lemma_binom_row_monotone(m - 1, k);
        lemma_binom_row_between(n, m - 1, k);
    }
}

/// Walk from column i to the middle of the row, using L6 at each step.
proof fn lemma_binom_k_chain(n: int, k: int, i: int)
    requires 34 <= i <= k, n == 2 * k,
    ensures spec_binom(n, i) <= spec_binom(n, k),
    decreases k - i,
{
    if i < k {
        assert(i + 1 <= k);
        assert(k <= n - i);
        lemma_binom_monotone(n, i);
        lemma_binom_k_chain(n, k, i + 1);
    }
}

/// L8: any reduced k of 34 or more exceeds u64.
proof fn lemma_binom_cap(k: int)
    requires 34 <= k,
    ensures spec_binom(2 * k, k) > max_u64(),
{
    assert(spec_binom(68, 34) > max_u64()) by (compute_only);
    lemma_binom_row_between(68, 2 * k, 34);
    lemma_binom_k_chain(2 * k, k, 34);
}


/// The largest `u128` value, as an `int`.
pub open spec fn max_u128() -> int { 0xffff_ffff_ffff_ffff_ffff_ffff_ffff_ffff }

/// `u64::MAX` as a `u128`, the threshold the running coefficient must not
/// pass.
const LIMIT_U64_IN_U128: u128 = 0xffff_ffff_ffff_ffff;

/// Walk from column i to the middle of any row, using L6 at each step. Unlike
/// `lemma_binom_k_chain` this does not require `n == 2 * k`: the kernel reduces
/// to `k = min(r, n - r)`, which is at or before the middle of a longer row.
proof fn lemma_binom_chain(n: int, k: int, i: int)
    requires 0 <= i <= k, 2 * k <= n,
    ensures spec_binom(n, i) <= spec_binom(n, k),
    decreases k - i,
{
    if i < k {
        assert(i + 1 <= k);
        assert(k <= n - k);
        assert(i + 1 <= n - i);
        lemma_binom_monotone(n, i);
        lemma_binom_chain(n, k, i + 1);
    }
}

/// The reduced index selects the same binomial coefficient as the original.
proof fn lemma_binom_reduced(n: int, r: int, k: int)
    requires 0 <= r <= n, k == (if r < n - r { r } else { n - r }),
    ensures spec_binom(n, r) == spec_binom(n, k),
{
    if r < n - r {
        assert(k == r);
    } else {
        assert(k == n - r);
        lemma_binom_symmetry(n, r);
    }
}

/// A product of two `u64` values always fits in a `u128`.
proof fn lemma_product_fits_u128(a: int, b: int)
    requires 0 <= a <= max_u64(), 0 <= b <= max_u64(),
    ensures a * b <= max_u128(),
{
    assert(max_u64() * max_u64() < max_u128()) by (compute_only);
    lemma_mul_inequality(a, max_u64(), b);
    lemma_mul_inequality(b, max_u64(), max_u64());
    lemma_mul_is_commutative(b, max_u64());
}

/// Return the exact binomial coefficient `n choose r`, or `None` on overflow.
fn checked_binomial(n: u64, r: u64) -> (result: Option<u64>)
    ensures
        match result {
            Option::Some(v) => v as int == spec_binom(n as int, r as int)
                && v as int <= max_u64(),
            Option::None => spec_binom(n as int, r as int) > max_u64(),
        },
{
    if r > n {
        proof {
            assert(spec_binom(n as int, r as int) == 0) by {
                reveal_with_fuel(spec_binom, 2);
            }
        }
        return Option::Some(0);
    }
    // The kernel writes `r.min(n - r)`; `min` has no Verus specification, so the
    // transcription selects the smaller index with an explicit branch.
    let k: u64 = if r < n - r { r } else { n - r };
    proof {
        lemma_binom_reduced(n as int, r as int, k as int);
    }
    if k >= 34 {
        proof {
            assert(LIMIT_U64_IN_U128 as int == max_u64()) by (compute_only);
            lemma_binom_cap(k as int);
            lemma_binom_row_between(2 * (k as int), n as int, k as int);
            lemma_binom_chain(n as int, k as int, k as int);
        }
        return Option::None;
    }
    proof {
        assert(2 * (k as int) <= n as int);
        assert(k as int <= 33);
        assert(LIMIT_U64_IN_U128 as int == max_u64()) by (compute_only);
    }
    let mut coefficient: u128 = 1;
    let mut i: u64 = 0;
    while i < k
        invariant
            i <= k,
            k as int <= 33,
            2 * (k as int) <= n as int,
            spec_binom(n as int, r as int) == spec_binom(n as int, k as int),
            (coefficient as int) == spec_binom(n as int, i as int),
            (coefficient as int) <= max_u64(),
        decreases k - i,
    {
        let factor: u64 = n - i;
        proof {
            assert((factor as int) == (n as int) - (i as int));
            assert((i as int) + 1 <= n as int);
            lemma_product_fits_u128(coefficient as int, factor as int);
        }
        let scaled: u128 = coefficient * (factor as u128);
        proof {
            assert((scaled as int) == (coefficient as int) * (factor as int));
            lemma_binom_nonneg(n as int, i as int + 1);
            lemma_binom_step(n as int, i as int);
            lemma_div_by_multiple(
                spec_binom(n as int, i as int + 1), (i as int) + 1,
            );
        }
        let divisor: u128 = (i + 1) as u128;
        coefficient = scaled / divisor;
        i = i + 1;
        proof {
            assert((divisor as int) == (i as int));
        }
        if coefficient > LIMIT_U64_IN_U128 {
            proof {
                assert(LIMIT_U64_IN_U128 as int == max_u64()) by (compute_only);
                assert((coefficient as int) > max_u64());
                lemma_binom_chain(n as int, k as int, i as int);
                assert(spec_binom(n as int, i as int)
                    <= spec_binom(n as int, k as int));
                assert(spec_binom(n as int, r as int) > max_u64());
            }
            return Option::None;
        }
    }
    proof {
        assert(LIMIT_U64_IN_U128 as int == max_u64()) by (compute_only);
        assert((coefficient as int) <= max_u64());
    }
    Option::Some(coefficient as u64)
}
}
