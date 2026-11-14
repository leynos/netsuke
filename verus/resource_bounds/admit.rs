//! Executable transcription of the running-total admission and classification.
//!
//! `admit_total` accepts an amount only when the sum stays within the ceiling,
//! and `classify` turns an optional exact cardinality into either the admitted
//! value or the typed refusal the caller reports.
use vstd::prelude::*;

verus! {

use crate::model::max_u64;

/// Why a combinatorial request was refused. Mirrors `CardinalityError`.
pub enum CardinalityError {
    Exceeded { cardinality: u64, ceiling: u64 },
    Overflow { ceiling: u64 },
}

/// Return the new running total when `amount` keeps it within `limit`.
fn admit_total(limit: u64, total: u64, amount: u64) -> (result: Option<u64>)
    ensures
        match result {
            Option::Some(v) => v as int == total as int + amount as int
                && v as int <= limit as int,
            Option::None => total as int + amount as int > limit as int
                || total as int + amount as int > max_u64(),
        },
{
    // The kernel writes `total.checked_add(amount).filter(...)`; `filter` has no
    // Verus specification, so the transcription spells the predicate out.
    match total.checked_add(amount) {
        Option::Some(sum) => {
            if sum <= limit {
                Option::Some(sum)
            } else {
                proof {
                    assert((limit as int) <= max_u64());
                }
                Option::None
            }
        }
        Option::None => {
            proof {
                assert((total as int) + (amount as int) > max_u64());
                assert((limit as int) <= max_u64());
            }
            Option::None
        }
    }
}

/// Classify an exact cardinality against the ceiling for its request kind.
fn classify(cardinality: Option<u64>, ceiling: u64) -> (result: Result<u64, CardinalityError>)
    ensures
        match result {
            Result::Ok(v) => cardinality == Option::Some(v)
                && (v as int) <= (ceiling as int),
            Result::Err(CardinalityError::Overflow { ceiling: reported }) =>
                cardinality is None && reported == ceiling,
            Result::Err(CardinalityError::Exceeded {
                cardinality: reported,
                ceiling: reported_ceiling,
            }) =>
                cardinality == Option::Some(reported)
                    && (reported as int) > (reported_ceiling as int)
                    && reported_ceiling == ceiling,
        },
{
    match cardinality {
        Option::None => Result::Err(CardinalityError::Overflow { ceiling }),
        Option::Some(value) => {
            if value > ceiling {
                Result::Err(CardinalityError::Exceeded { cardinality: value, ceiling })
            } else {
                Result::Ok(value)
            }
        }
    }
}
}
