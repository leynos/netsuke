//! Executable composition of each cardinality kernel with `classify`.
//!
//! The plan's contract theorem has two halves: what a kernel returns for its
//! exact count, and what classification makes of that count. Proving the halves
//! separately does not prove the pair, and the pair is what a caller actually
//! reaches, so each kernel is composed with `classify` here and the composed
//! postcondition states the three outcomes directly: an admitted count at or
//! below the ceiling, an exceeded refusal for a representable count above it,
//! and an overflow refusal only when the true count is unrepresentable.
use vstd::prelude::*;

verus! {

use crate::admit::{classify, CardinalityError};
use crate::model::{max_u64, spec_binom, spec_falling, spec_product};

/// Classify the `product` tuple count against `ceiling`.
fn classify_product(lengths: &Vec<usize>, repeat: u64, ceiling: u64)
    -> (result: Result<u64, CardinalityError>)
    ensures
        match result {
            Result::Ok(v) => v as int == spec_product(lengths@, repeat as int)
                && v as int <= ceiling as int,
            Result::Err(CardinalityError::Exceeded {
                cardinality,
                ceiling: reported,
            }) =>
                cardinality as int == spec_product(lengths@, repeat as int)
                    && cardinality as int > reported as int
                    && reported == ceiling,
            Result::Err(CardinalityError::Overflow { ceiling: reported }) =>
                spec_product(lengths@, repeat as int) > max_u64()
                    && reported == ceiling,
        },
{
    classify(crate::power_product::checked_product(lengths, repeat), ceiling)
}

/// Classify the `combinations` cardinality against `ceiling`.
fn classify_binomial(n: u64, r: u64, ceiling: u64)
    -> (result: Result<u64, CardinalityError>)
    ensures
        match result {
            Result::Ok(v) => v as int == spec_binom(n as int, r as int)
                && v as int <= ceiling as int,
            Result::Err(CardinalityError::Exceeded {
                cardinality,
                ceiling: reported,
            }) =>
                cardinality as int == spec_binom(n as int, r as int)
                    && cardinality as int > reported as int
                    && reported == ceiling,
            Result::Err(CardinalityError::Overflow { ceiling: reported }) =>
                spec_binom(n as int, r as int) > max_u64()
                    && reported == ceiling,
        },
{
    classify(crate::binomial::checked_binomial(n, r), ceiling)
}

/// Classify the `permutations` cardinality against `ceiling`.
fn classify_falling(n: u64, r: u64, ceiling: u64)
    -> (result: Result<u64, CardinalityError>)
    ensures
        match result {
            Result::Ok(v) => v as int == spec_falling(n as int, r as int)
                && v as int <= ceiling as int,
            Result::Err(CardinalityError::Exceeded {
                cardinality,
                ceiling: reported,
            }) =>
                cardinality as int == spec_falling(n as int, r as int)
                    && cardinality as int > reported as int
                    && reported == ceiling,
            Result::Err(CardinalityError::Overflow { ceiling: reported }) =>
                spec_falling(n as int, r as int) > max_u64()
                    && reported == ceiling,
        },
{
    classify(crate::falling::checked_falling_factorial(n, r), ceiling)
}

}