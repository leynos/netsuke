//! The abstract model shared by the kernel proofs.
//!
//! Each specification states the mathematical value a kernel computes, so the
//! kernel postconditions can be written against it rather than against the
//! implementation's arithmetic.
use vstd::prelude::*;
use vstd::arithmetic::mul::*;
use vstd::arithmetic::div_mod::*;

verus! {

/// The largest `u64` value, as an `int`.
pub open spec fn max_u64() -> int { 0xffff_ffff_ffff_ffff }

// ---------------------------------------------------------------- model ----

/// `base` raised to `repeat`, with `repeat <= 0` giving one.
pub open spec fn spec_power(base: int, repeat: int) -> int
    decreases repeat,
{
    if repeat <= 0 { 1 } else { base * spec_power(base, repeat - 1) }
}

/// The product of the first `end` operand lengths, one for an empty prefix.
pub open spec fn spec_lens_product(lens: Seq<usize>, end: int) -> int
    decreases end,
{
    if end <= 0 { 1 } else { spec_lens_product(lens, end - 1) * (lens[end - 1] as int) }
}

/// The product of every operand length.
pub open spec fn spec_lens_total(lens: Seq<usize>) -> int {
    spec_lens_product(lens, lens.len() as int)
}

/// The exact tuple count of a `product` request.
pub open spec fn spec_product(lens: Seq<usize>, repeat: int) -> int {
    spec_power(spec_lens_total(lens), repeat)
}

/// The falling factorial `n * (n - 1) * ... * (n - r + 1)`, zero when
/// `r > n`.
pub open spec fn spec_falling(n: int, r: int) -> int
    decreases r,
{
    if r <= 0 { 1 }
    else if n < r { 0 }
    else { n * spec_falling(n - 1, r - 1) }
}

#[verifier::memoize]
pub open spec fn spec_binom(n: int, r: int) -> int
    decreases n,
{
    if n < 0 || r < 0 || r > n { 0 }
    else if r == 0 { 1 }
    else if r == n { 1 }
    else { spec_binom(n - 1, r - 1) + spec_binom(n - 1, r) }
}

/// Does any operand before `end` equal zero?
pub open spec fn spec_has_zero(lens: Seq<usize>, end: int) -> bool
    decreases end,
{
    if end <= 0 { false } else { lens[end - 1] == 0 || spec_has_zero(lens, end - 1) }
}

/// Is every operand before `end` at least one?
pub open spec fn spec_all_positive(lens: Seq<usize>, end: int) -> bool
    decreases end,
{
    if end <= 0 { true } else { lens[end - 1] >= 1 && spec_all_positive(lens, end - 1) }
}

// ------------------------------------------------- ring helper lemmas ----

/// Multiplying by a successor adds one more copy: `(m + 1) * x == m * x + x`.
pub proof fn lemma_mul_succ_left(m: int, x: int)
    ensures (m + 1) * x == m * x + x,
{
    lemma_mul_is_commutative(m + 1, x);
    lemma_mul_is_distributive_add(x, m, 1);
    lemma_mul_is_commutative(m, x);
    assert(x * 1 == x) by (nonlinear_arith);
}

/// The product of two non-negative integers is non-negative.
pub proof fn lemma_mul_nonneg(a: int, b: int)
    requires a >= 0, b >= 0,
    ensures a * b >= 0,
{
    lemma_mul_inequality(0, b, a);
    assert(a * 0 == 0) by (nonlinear_arith);
}

// ---------------------------------------------------------- power lemmas ----

/// Any base raised to zero is one.
pub proof fn lemma_pow_zero(base: int)
    ensures spec_power(base, 0) == 1,
{
}

/// Any base raised to one is itself.
pub proof fn lemma_pow_one(base: int)
    ensures spec_power(base, 1) == base,
{
    reveal_with_fuel(spec_power, 2);
    assert(spec_power(base, 1) == base * 1);
}

/// A non-negative base raised to any exponent stays non-negative.
pub proof fn lemma_pow_nonneg(base: int, repeat: int)
    requires base >= 0,
    ensures spec_power(base, repeat) >= 0,
    decreases repeat,
{
    if repeat > 0 {
        lemma_pow_nonneg(base, repeat - 1);
        assert(spec_power(base, repeat) == base * spec_power(base, repeat - 1)) by {
            reveal_with_fuel(spec_power, 2);
        }
        lemma_mul_nonneg(base, spec_power(base, repeat - 1));
    }
}

/// Exponents add: `base^(a + b) == base^a * base^b`.
pub proof fn lemma_pow_add(base: int, a: int, b: int)
    requires 0 <= a, 0 <= b,
    ensures spec_power(base, a + b) == spec_power(base, a) * spec_power(base, b),
    decreases b,
{
    if b > 0 {
        lemma_pow_add(base, a, b - 1);
        assert(spec_power(base, a + b) == base * spec_power(base, a + b - 1)) by {
            reveal_with_fuel(spec_power, 2);
        }
        assert(base * spec_power(base, a + b - 1)
            == base * (spec_power(base, a) * spec_power(base, b - 1)));
        assert(base * (spec_power(base, a) * spec_power(base, b - 1))
            == (base * spec_power(base, a)) * spec_power(base, b - 1)) by {
            lemma_mul_is_associative(base, spec_power(base, a), spec_power(base, b - 1));
        }
        assert(base * spec_power(base, a) == spec_power(base, a) * base) by {
            lemma_mul_is_commutative(base, spec_power(base, a));
        }
        assert((spec_power(base, a) * base) * spec_power(base, b - 1)
            == spec_power(base, a) * (base * spec_power(base, b - 1))) by {
            lemma_mul_is_associative(spec_power(base, a), base, spec_power(base, b - 1));
        }
        assert(base * spec_power(base, b - 1) == spec_power(base, b)) by {
            reveal_with_fuel(spec_power, 2);
        }
    } else {
        lemma_pow_zero(base);
    }
}

/// A base of at least one does not lose value as the exponent grows.
pub proof fn lemma_pow_mono_exp(base: int, a: int, b: int)
    requires base >= 1, 0 <= a <= b,
    ensures spec_power(base, a) <= spec_power(base, b),
    decreases b,
{
    if b > a {
        lemma_pow_mono_exp(base, a, b - 1);
        lemma_pow_nonneg(base, b - 1);
        assert(spec_power(base, b) == base * spec_power(base, b - 1)) by {
            reveal_with_fuel(spec_power, 2);
        }
        assert(spec_power(base, b - 1) * 1 <= spec_power(base, b - 1) * base) by {
            lemma_mul_inequality(1, base, spec_power(base, b - 1));
        }
        assert(spec_power(base, b - 1) * 1 == spec_power(base, b - 1)) by (nonlinear_arith);
        assert(spec_power(base, b - 1) <= base * spec_power(base, b - 1)) by {
            lemma_mul_is_commutative(spec_power(base, b - 1), base);
        }
    }
}

/// A larger base yields at least as large a power.
pub proof fn lemma_pow_mono_base(base: int, other: int, repeat: int)
    requires 1 <= base <= other, 0 <= repeat,
    ensures spec_power(base, repeat) <= spec_power(other, repeat),
    decreases repeat,
{
    if repeat > 0 {
        lemma_pow_mono_base(base, other, repeat - 1);
        lemma_pow_nonneg(base, repeat - 1);
        lemma_pow_nonneg(other, repeat - 1);
        assert(spec_power(base, repeat) == base * spec_power(base, repeat - 1)) by {
            reveal_with_fuel(spec_power, 2);
        }
        assert(spec_power(other, repeat) == other * spec_power(other, repeat - 1)) by {
            reveal_with_fuel(spec_power, 2);
        }
        assert(base * spec_power(base, repeat - 1)
            <= base * spec_power(other, repeat - 1)) by {
            lemma_mul_inequality(
                spec_power(base, repeat - 1),
                spec_power(other, repeat - 1),
                base,
            );
        }
        assert(base * spec_power(other, repeat - 1)
            <= other * spec_power(other, repeat - 1)) by {
            lemma_mul_inequality(base, other, spec_power(other, repeat - 1));
        }
    }
}

/// `2^64` is exactly `u64::MAX + 1`, the first power of two that overflows.
pub proof fn lemma_two_pow_64()
    ensures spec_power(2, 64) == max_u64() + 1,
{
    reveal_with_fuel(spec_power, 65);
}

/// L3: a base of at least two and an exponent of at least 64 overflows.
pub proof fn lemma_pow_overflow(base: int, repeat: int)
    requires base >= 2, repeat >= 64,
    ensures spec_power(base, repeat) > max_u64(),
{
    lemma_two_pow_64();
    lemma_pow_mono_base(2, base, 64);
    lemma_pow_mono_exp(base, 64, repeat);
}

/// Zero raised to a positive exponent is zero.
pub proof fn lemma_pow_zero_base(repeat: int)
    requires repeat >= 1,
    ensures spec_power(0, repeat) == 0,
    decreases repeat,
{
    if repeat > 1 {
        lemma_pow_zero_base(repeat - 1);
    }
    assert(spec_power(0, repeat) == 0 * spec_power(0, repeat - 1)) by {
        reveal_with_fuel(spec_power, 2);
    }
}

/// One raised to any exponent is one.
pub proof fn lemma_pow_one_base(repeat: int)
    requires repeat >= 0,
    ensures spec_power(1, repeat) == 1,
    decreases repeat,
{
    if repeat > 0 {
        lemma_pow_one_base(repeat - 1);
    }
    assert(spec_power(1, repeat) == 1 * spec_power(1, repeat - 1)) by {
        reveal_with_fuel(spec_power, 2);
    }
}
}
