//! Executable transcription of the `product` kernel.
//!
//! `checked_product` scans for a zero operand, multiplies the lengths, and
//! raises the resulting base to the requested repeat count. Its postcondition
//! is the exact tuple count or the exact overflow.
use vstd::prelude::*;
use vstd::arithmetic::mul::*;
use vstd::arithmetic::div_mod::*;
use crate::model::*;
use crate::lens_product::*;

verus! {

// ---------------------------------------------------------- the kernels ----

/// Raise `base` to `repeat`, refusing a result above `u64::MAX`.
fn checked_power(base: u64, repeat: u64) -> (result: Option<u64>)
    requires repeat >= 1,
    ensures
        match result {
            Option::Some(v) => v as int == spec_power(base as int, repeat as int)
                && v as int <= max_u64(),
            Option::None => spec_power(base as int, repeat as int) > max_u64(),
        },
{
    if base <= 1 {
        proof {
            if base == 0 {
                lemma_pow_zero_base(repeat as int);
            } else {
                lemma_pow_one_base(repeat as int);
            }
        }
        return Option::Some(base);
    }
    if repeat >= 64 {
        proof {
            lemma_pow_overflow(base as int, repeat as int);
        }
        return Option::None;
    }
    // The kernel calls `u64::checked_pow`; Verus has no specification for it,
    // so the transcription uses an explicit fold of `checked_mul`.
    let mut accumulator: u64 = 1;
    let mut exponent: u64 = 0;
    while exponent < repeat
        invariant
            exponent <= repeat,
            (accumulator as int) == spec_power(base as int, exponent as int),
        decreases repeat - exponent,
    {
        match accumulator.checked_mul(base) {
            Option::Some(product) => {
                proof {
                    assert(product as int == accumulator as int * base as int);
                    assert(spec_power(base as int, exponent as int + 1)
                        == base as int * spec_power(base as int, exponent as int)) by {
                        reveal_with_fuel(spec_power, 2);
                    }
                    assert(spec_power(base as int, exponent as int + 1)
                        == base as int * spec_power(base as int, exponent as int));
                    assert(spec_power(base as int, exponent as int) == accumulator as int);
                    assert(spec_power(base as int, exponent as int + 1)
                        == base as int * accumulator as int);
                    assert(base as int * accumulator as int
                        == accumulator as int * base as int) by {
                        lemma_mul_is_commutative(base as int, accumulator as int);
                    }
                    assert(product as int == accumulator as int * base as int);
                    assert(accumulator as int * base as int
                        == spec_power(base as int, exponent as int + 1));
                }
                accumulator = product;
                exponent = exponent + 1;
                proof {
                    assert(accumulator as int
                        == spec_power(base as int, exponent as int));
                }
            }
            Option::None => {
                proof {
                    lemma_pow_nonneg(base as int, exponent as int);
                    assert(spec_power(base as int, exponent as int + 1)
                        == base as int * spec_power(base as int, exponent as int)) by {
                        reveal_with_fuel(spec_power, 2);
                    }
                    assert(spec_power(base as int, exponent as int + 1)
                        == accumulator as int * base as int) by {
                        lemma_mul_is_commutative(
                            base as int, spec_power(base as int, exponent as int),
                        );
                    }
                    lemma_pow_mono_exp(base as int, exponent as int + 1, repeat as int);
                }
                return Option::None;
            }
        }
    }
    Option::Some(accumulator)
}

/// Return the exact tuple count of a `product` request, or `None` on overflow.
fn checked_product(lengths: &Vec<usize>, repeat: u64) -> (result: Option<u64>)
    requires repeat >= 1,
    ensures
        match result {
            Option::Some(v) => v as int == spec_product(lengths@, repeat as int),
            Option::None => spec_product(lengths@, repeat as int) > max_u64(),
        },
{
    let mut index: usize = 0;
    let mut has_zero = false;
    while index < lengths.len()
        invariant
            index <= lengths.len(),
            has_zero ==> spec_has_zero(lengths@, index as int),
            !has_zero ==> !spec_has_zero(lengths@, index as int),
        decreases lengths.len() - index,
    {
        proof {
            assert(spec_has_zero(lengths@, index as int + 1)
                == (lengths[index as int] == 0 || spec_has_zero(lengths@, index as int))) by {
                reveal_with_fuel(spec_has_zero, 2);
            }
        }
        if lengths[index] == 0 {
            has_zero = true;
        }
        index = index + 1;
    }
    if has_zero {
        proof {
            lemma_lens_product_zero(lengths@, lengths.len() as int);
            lemma_pow_zero_base(repeat as int);
        }
        return Option::Some(0);
    }
    proof {
        lemma_no_zero_all_positive(lengths@, lengths.len() as int);
    }
    let mut base: u64 = 1;
    let mut position: usize = 0;
    while position < lengths.len()
        invariant
            position <= lengths.len(),
            (base as int) == spec_lens_product(lengths@, position as int),
            spec_all_positive(lengths@, lengths.len() as int),
            (repeat as int) >= 1,
        decreases lengths.len() - position,
    {
        let factor = lengths[position] as u64;
        match base.checked_mul(factor) {
            Option::Some(product) => {
                proof {
                    assert(spec_lens_product(lengths@, position as int + 1)
                        == spec_lens_product(lengths@, position as int)
                            * (lengths[position as int] as int)) by {
                        reveal_with_fuel(spec_lens_product, 2);
                    }
                }
                base = product;
                position = position + 1;
            }
            Option::None => {
                proof {
                    assert(spec_lens_product(lengths@, position as int + 1)
                        == spec_lens_product(lengths@, position as int)
                            * (lengths[position as int] as int)) by {
                        reveal_with_fuel(spec_lens_product, 2);
                    }
                    assert(spec_lens_product(lengths@, position as int + 1) > max_u64());
                    lemma_lens_product_monotone(
                        lengths@,
                        position as int + 1,
                        lengths.len() as int,
                    );
                    lemma_all_positive_prefix(
                        lengths@, lengths.len() as int, position as int + 1,
                    );
                    lemma_lens_product_positive(lengths@, position as int + 1);
                    assert(repeat as int >= 1);
                    lemma_pow_mono_exp(spec_lens_total(lengths@), 1, repeat as int);
                    lemma_pow_one(spec_lens_total(lengths@));
                    assert(spec_lens_total(lengths@)
                        == spec_lens_product(lengths@, lengths.len() as int));
                    assert(spec_lens_product(lengths@, position as int + 1)
                        <= spec_lens_total(lengths@));
                    assert(spec_product(lengths@, repeat as int)
                        == spec_power(spec_lens_total(lengths@), repeat as int));
                    assert(spec_power(spec_lens_total(lengths@), 1)
                        <= spec_power(spec_lens_total(lengths@), repeat as int));
                    assert(spec_power(spec_lens_total(lengths@), 1)
                        == spec_lens_total(lengths@));
                    assert(spec_product(lengths@, repeat as int)
                        >= spec_lens_total(lengths@));
                    assert(spec_lens_total(lengths@)
                        >= spec_lens_product(lengths@, position as int + 1));
                    assert(spec_lens_total(lengths@) > max_u64());
                }
                return Option::None;
            }
        }
    }
    match checked_power(base, repeat) {
        Option::Some(value) => Option::Some(value),
        Option::None => Option::None,
    }
}
}
