//! Lemmas about the `product` tuple count.
//!
//! A zero operand absorbs the whole product (L1) and, with no zero operand,
//! every prefix product is at least one and never shrinks (L2). Together they
//! justify both the early `Some(0)` and the in-loop `None` of the kernel.
use vstd::prelude::*;
use vstd::arithmetic::mul::*;
use vstd::arithmetic::div_mod::*;
use crate::model::*;

verus! {

/// A prefix product of operand lengths is never negative.
pub proof fn lemma_lens_product_nonneg(lens: Seq<usize>, end: int)
    requires 0 <= end <= lens.len(),
    ensures spec_lens_product(lens, end) >= 0,
    decreases end,
{
    if end > 0 {
        lemma_lens_product_nonneg(lens, end - 1);
        assert(spec_lens_product(lens, end)
            == spec_lens_product(lens, end - 1) * (lens[end - 1] as int)) by {
            reveal_with_fuel(spec_lens_product, 2);
        }
        lemma_mul_nonneg(spec_lens_product(lens, end - 1), lens[end - 1] as int);
    }
}

/// L1: an operand of zero absorbs every prefix and the whole product.
pub proof fn lemma_lens_product_zero(lens: Seq<usize>, end: int)
    requires 0 <= end <= lens.len(), spec_has_zero(lens, end),
    ensures spec_lens_product(lens, end) == 0,
    decreases end,
{
    if end > 0 {
        if lens[end - 1] == 0 {
            lemma_lens_product_nonneg(lens, end - 1);
            lemma_mul_basics(spec_lens_product(lens, end - 1));
            assert(spec_lens_product(lens, end)
                == spec_lens_product(lens, end - 1) * (lens[end - 1] as int)) by {
                reveal_with_fuel(spec_lens_product, 2);
            }
        } else {
            assert(spec_has_zero(lens, end - 1)) by {
                reveal_with_fuel(spec_has_zero, 2);
            }
            lemma_lens_product_zero(lens, end - 1);
            lemma_mul_basics(lens[end - 1] as int);
            assert(spec_lens_product(lens, end)
                == spec_lens_product(lens, end - 1) * (lens[end - 1] as int)) by {
                reveal_with_fuel(spec_lens_product, 2);
            }
        }
    }
}

/// L2: with no zero operand the prefix products never shrink.
pub proof fn lemma_lens_product_monotone(lens: Seq<usize>, i: int, end: int)
    requires 0 <= i <= end <= lens.len(), spec_all_positive(lens, end),
    ensures spec_lens_product(lens, i) <= spec_lens_product(lens, end),
    decreases end,
{
    if i < end {
        assert(spec_all_positive(lens, end - 1)) by {
            reveal_with_fuel(spec_all_positive, 2);
        }
        lemma_lens_product_nonneg(lens, end - 1);
        lemma_lens_product_monotone(lens, i, end - 1);
        assert(spec_lens_product(lens, end)
            == spec_lens_product(lens, end - 1) * (lens[end - 1] as int)) by {
            reveal_with_fuel(spec_lens_product, 2);
        }
        assert(spec_lens_product(lens, end - 1) * 1
            <= spec_lens_product(lens, end - 1) * (lens[end - 1] as int)) by {
            lemma_mul_inequality(1, lens[end - 1] as int, spec_lens_product(lens, end - 1));
        }
        lemma_mul_basics(spec_lens_product(lens, end - 1));
    }
}

/// The kernel's justification for returning `None` at an overflowing prefix.
pub proof fn lemma_lens_total_above(lens: Seq<usize>, i: int)
    requires 0 <= i <= lens.len(), spec_all_positive(lens, lens.len() as int),
        spec_lens_product(lens, i) > max_u64(),
    ensures spec_lens_total(lens) > max_u64(),
{
    lemma_lens_product_monotone(lens, i, lens.len() as int);
}

/// Zero absorption lifts from a prefix to the whole operand list.
pub proof fn lemma_lens_total_zero(lens: Seq<usize>)
    requires spec_has_zero(lens, lens.len() as int),
    ensures spec_lens_total(lens) == 0,
{
    lemma_lens_product_zero(lens, lens.len() as int);
}

/// A positive-operand prefix of a positive-operand range is positive.
pub proof fn lemma_all_positive_prefix(lens: Seq<usize>, end: int, i: int)
    requires 0 <= i <= end, spec_all_positive(lens, end),
    ensures spec_all_positive(lens, i),
    decreases end,
{
    if i < end {
        assert(spec_all_positive(lens, end - 1)) by {
            reveal_with_fuel(spec_all_positive, 2);
        }
        lemma_all_positive_prefix(lens, end - 1, i);
    }
}

/// With no zero operand every prefix product is at least one.
pub proof fn lemma_lens_product_positive(lens: Seq<usize>, end: int)
    requires 0 <= end <= lens.len(), spec_all_positive(lens, end),
    ensures spec_lens_product(lens, end) >= 1,
    decreases end,
{
    if end > 0 {
        assert(spec_all_positive(lens, end - 1)) by {
            reveal_with_fuel(spec_all_positive, 2);
        }
        lemma_lens_product_positive(lens, end - 1);
        lemma_lens_product_nonneg(lens, end - 1);
        assert(spec_lens_product(lens, end)
            == spec_lens_product(lens, end - 1) * (lens[end - 1] as int)) by {
            reveal_with_fuel(spec_lens_product, 2);
        }
        assert(spec_lens_product(lens, end - 1) * 1
            <= spec_lens_product(lens, end - 1) * (lens[end - 1] as int)) by {
            lemma_mul_inequality(1, lens[end - 1] as int, spec_lens_product(lens, end - 1));
        }
        lemma_mul_basics(spec_lens_product(lens, end - 1));
    }
}

/// No zero operand is the same as every operand being at least one.
pub proof fn lemma_no_zero_all_positive(lens: Seq<usize>, end: int)
    requires !spec_has_zero(lens, end),
    ensures spec_all_positive(lens, end),
    decreases end,
{
    if end > 0 {
        assert(!spec_has_zero(lens, end - 1)) by {
            reveal_with_fuel(spec_has_zero, 2);
        }
        assert(!spec_has_zero(lens, end - 1) && lens[end - 1] != 0) by {
            reveal_with_fuel(spec_has_zero, 2);
        }
        lemma_no_zero_all_positive(lens, end - 1);
    }
}
}
