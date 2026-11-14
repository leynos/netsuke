//! Lemmas and transcription for the falling-factorial cardinality.
//!
//! The falling factorial is monotone in both arguments, and every value with
//! `r >= 21` exceeds `u64` (L9). The transcription follows the kernel's loop,
//! which multiplies one factor at a time and refuses on overflow.
use vstd::prelude::*;
use vstd::arithmetic::mul::*;
use vstd::arithmetic::div_mod::*;
use crate::model::*;

verus! {

/// A falling factorial is never negative.
proof fn lemma_falling_nonneg(n: int, r: int)
    requires n >= r, r >= 0,
    ensures spec_falling(n, r) >= 0,
    decreases r,
{
    if r > 0 {
        lemma_falling_nonneg(n - 1, r - 1);
        assert(spec_falling(n, r) == n * spec_falling(n - 1, r - 1)) by {
            reveal_with_fuel(spec_falling, 2);
        }
        lemma_mul_inequality(0, spec_falling(n - 1, r - 1), n);
        assert(n * 0 == 0) by (nonlinear_arith);
    }
}

/// L7: monotonicity in n at fixed r.
proof fn lemma_falling_mono_n(n: int, r: int)
    requires r >= 0, r <= n,
    ensures spec_falling(n, r) <= spec_falling(n + 1, r),
    decreases r,
{
    if r == 0 {
        assert(spec_falling(n, 0) == 1) by { reveal_with_fuel(spec_falling, 2); }
        assert(spec_falling(n + 1, 0) == 1) by { reveal_with_fuel(spec_falling, 2); }
    } else {
        lemma_falling_mono_n(n - 1, r - 1);
        lemma_falling_nonneg(n - 1, r - 1);
        lemma_falling_nonneg(n, r - 1);
        assert(spec_falling(n, r) == n * spec_falling(n - 1, r - 1)) by {
            reveal_with_fuel(spec_falling, 2);
        }
        assert(spec_falling(n + 1, r) == (n + 1) * spec_falling(n, r - 1)) by {
            reveal_with_fuel(spec_falling, 2);
        }
        // `lemma_mul_inequality` multiplies on the right, so commute the
        // first factor into place before and after each application.
        assert(spec_falling(n - 1, r - 1) * n <= spec_falling(n, r - 1) * n) by {
            lemma_mul_inequality(spec_falling(n - 1, r - 1), spec_falling(n, r - 1), n);
        }
        assert(n * spec_falling(n - 1, r - 1) == spec_falling(n - 1, r - 1) * n) by {
            lemma_mul_is_commutative(n, spec_falling(n - 1, r - 1));
        }
        assert(n * spec_falling(n, r - 1) == spec_falling(n, r - 1) * n) by {
            lemma_mul_is_commutative(n, spec_falling(n, r - 1));
        }
        assert(n * spec_falling(n, r - 1) <= (n + 1) * spec_falling(n, r - 1)) by {
            lemma_mul_inequality(n, n + 1, spec_falling(n, r - 1));
        }
    }
}

/// The factorial grows: r! <= (r+1)!.
proof fn lemma_factorial_mono(r: int)
    requires r >= 0,
    ensures spec_falling(r, r) <= spec_falling(r + 1, r + 1),
{
    lemma_falling_nonneg(r, r);
    assert(spec_falling(r + 1, r + 1) == (r + 1) * spec_falling(r, r)) by {
        reveal_with_fuel(spec_falling, 2);
    }
    assert(spec_falling(r, r) <= (r + 1) * spec_falling(r, r)) by {
        lemma_mul_inequality(1, r + 1, spec_falling(r, r));
    }
    assert(1 * spec_falling(r, r) == spec_falling(r, r));
}

/// `r!` is at least `21!` once `r` reaches 21.
proof fn lemma_factorial_chain(r: int)
    requires r >= 21,
    ensures spec_falling(21, 21) <= spec_falling(r, r),
    decreases r,
{
    if r > 21 {
        lemma_factorial_chain(r - 1);
        lemma_factorial_mono(r - 1);
    }
}

/// L9: for 21 <= r <= n the falling factorial exceeds u64.
proof fn lemma_falling_cap(n: int, r: int)
    requires 21 <= r <= n,
    ensures spec_falling(n, r) > max_u64(),
{
    assert(spec_falling(21, 21) > max_u64()) by (compute_only);
    lemma_falling_mono_n(r, r);
    lemma_factorial_chain(r);
    lemma_falling_chain(r, n);
}

/// A falling factorial never shrinks as its row grows at a fixed length.
proof fn lemma_falling_chain(r: int, n: int)
    requires 0 <= r <= n,
    ensures spec_falling(r, r) <= spec_falling(n, r),
    decreases n,
{
    if n > r {
        lemma_falling_mono_n(n - 1, r);
        lemma_falling_chain(r, n - 1);
    }
}

/// The multiplicative step the loop performs: dropping one factor of the
/// falling factorial is exactly one multiplication.
proof fn lemma_falling_step(n: int, i: int)
    requires 0 <= i < n,
    ensures spec_falling(n, i) * (n - i) == spec_falling(n, i + 1),
    decreases i,
{
    if i == 0 {
        assert(spec_falling(n, 0) == 1) by { reveal_with_fuel(spec_falling, 2); }
        assert(spec_falling(n - 1, 0) == 1) by { reveal_with_fuel(spec_falling, 2); }
        assert(spec_falling(n, 1) == n * spec_falling(n - 1, 0)) by {
            reveal_with_fuel(spec_falling, 2);
        }
        lemma_mul_basics(n);
    } else {
        lemma_falling_step(n - 1, i - 1);
        assert(spec_falling(n, i) == n * spec_falling(n - 1, i - 1)) by {
            reveal_with_fuel(spec_falling, 2);
        }
        assert(spec_falling(n, i + 1) == n * spec_falling(n - 1, i)) by {
            reveal_with_fuel(spec_falling, 2);
        }
        assert((n - 1) - (i - 1) == n - i);
        assert(spec_falling(n - 1, i - 1) * (n - i) == spec_falling(n - 1, i));
        assert(n * (spec_falling(n - 1, i - 1) * (n - i))
            == n * spec_falling(n - 1, i));
        assert((n * spec_falling(n - 1, i - 1)) * (n - i)
            == n * (spec_falling(n - 1, i - 1) * (n - i))) by {
            lemma_mul_is_associative(n, spec_falling(n - 1, i - 1), n - i);
        }
    }
}

/// Monotonicity in the second argument: a longer falling factorial of the same
/// row is never smaller.
proof fn lemma_falling_mono_r(n: int, a: int, b: int)
    requires 0 <= a <= b <= n,
    ensures spec_falling(n, a) <= spec_falling(n, b),
    decreases b - a,
{
    if a < b {
        lemma_falling_mono_r(n, a, b - 1);
        lemma_falling_nonneg(n, b - 1);
        lemma_falling_step(n, b - 1);
        assert(n - (b - 1) >= 1);
        lemma_mul_inequality(1, n - (b - 1), spec_falling(n, b - 1));
        lemma_mul_basics(spec_falling(n, b - 1));
        assert(1 * spec_falling(n, b - 1) == spec_falling(n, b - 1));
        assert((n - (b - 1)) * spec_falling(n, b - 1)
            == spec_falling(n, b - 1) * (n - (b - 1))) by {
            lemma_mul_is_commutative(n - (b - 1), spec_falling(n, b - 1));
        }
    }
}

/// Return the exact falling factorial `n * (n-1) * ... * (n-r+1)`, or `None`
/// when it exceeds `u64::MAX`.
fn checked_falling_factorial(n: u64, r: u64) -> (result: Option<u64>)
    ensures
        match result {
            Option::Some(v) => v as int == spec_falling(n as int, r as int)
                && v as int <= max_u64(),
            Option::None => spec_falling(n as int, r as int) > max_u64(),
        },
{
    if r > n {
        proof {
            assert(spec_falling(n as int, r as int) == 0) by {
                reveal_with_fuel(spec_falling, 2);
            }
        }
        return Option::Some(0);
    }
    if r >= 21 {
        proof {
            lemma_falling_cap(n as int, r as int);
        }
        return Option::None;
    }
    let mut product: u64 = 1;
    let mut i: u64 = 0;
    while i < r
        invariant
            i <= r,
            (r as int) <= (n as int),
            (product as int) == spec_falling(n as int, i as int),
        decreases r - i,
    {
        let factor: u64 = n - i;
        match product.checked_mul(factor) {
            Option::Some(next) => {
                proof {
                    assert(next as int == product as int * factor as int);
                    assert((factor as int) == (n as int) - (i as int));
                    assert((i as int) < (n as int));
                    lemma_falling_step(n as int, i as int);
                    assert(spec_falling(n as int, i as int) * ((n as int) - (i as int))
                        == spec_falling(n as int, i as int + 1));
                    assert(spec_falling(n as int, i as int + 1) == next as int);
                }
                product = next;
                i = i + 1;
            }
            Option::None => {
                proof {
                    lemma_falling_nonneg(n as int, i as int);
                    lemma_falling_step(n as int, i as int);
                    assert(spec_falling(n as int, i as int + 1)
                        == spec_falling(n as int, i as int) * ((n as int) - (i as int)));
                    assert(spec_falling(n as int, i as int + 1) > max_u64());
                    lemma_falling_mono_r(n as int, i as int + 1, r as int);
                }
                return Option::None;
            }
        }
    }
    Option::Some(product)
}
}
