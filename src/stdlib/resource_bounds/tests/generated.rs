//! The generated requests the property batteries draw from.
//!
//! The three cardinality kernels take different inputs, so a generated request
//! carries the shape it was drawn for and answers the two questions a battery
//! asks of it: what the production kernel says, and what the oracle says. That
//! is what lets one law body and one tripwire body serve every battery, while
//! each battery still runs its own strategy with its own case count, so no
//! battery's coverage can be masked by another's.
//!
//! The generators live here rather than beside the batteries so that the
//! shapes a battery can draw stay in one place, and so the battery module stays
//! within the repository's module-size limit.

use proptest::prelude::*;

use super::oracle;
use super::*;

/// The largest `n` the combination strategy draws from.
///
/// A Pascal row exists up to [`oracle::PASCAL_ROW_LIMIT`], so the oracle can
/// answer every combination this strategy generates by its own route.
const COMBINATION_MAX_LENGTH: usize = 130;

/// The largest `n` the permutation strategy draws from.
///
/// Chosen so the falling-factorial ceiling is reachable well inside the range:
/// a request below it is admitted and one above it is refused.
const PERMUTATION_MAX_LENGTH: usize = 40;

/// Generate one operand length: zero, one, or a power of two up to `2^20`.
///
/// Zero and one are common because they are the two lengths that decide the
/// product before any multiplication runs, which is exactly the branch the
/// witnesses pin and the property must reach independently.
fn operand_length() -> impl Strategy<Value = usize> {
    prop_oneof![
        1 => Just(0_usize),
        1 => Just(1_usize),
        8 => (0_u32..=20).prop_map(|exponent| 1_usize << exponent),
    ]
}

/// Generate up to six operand lengths for a product request.
fn product_request() -> impl Strategy<Value = (Vec<usize>, u64)> {
    (
        prop::collection::vec(operand_length(), 0..=6),
        prop_oneof![1 => Just(0_u64), 1 => Just(1_u64), 6 => 2_u64..=8],
    )
}

/// Generate a selection of at most `max_length` items that overshoots one time
/// in eight.
///
/// A combination and a permutation request are drawn the same way but graded by
/// different ceilings. The overshoot is what makes the `Zero` class reachable,
/// because it is the only way either cardinality is zero: a strategy confined
/// to `r <= n` would fail its class-coverage tripwire against a correct kernel.
fn overshooting_request(max_length: usize) -> impl Strategy<Value = (usize, u64)> {
    (0_usize..=max_length).prop_flat_map(move |length| {
        let width = oracle::width_of(length);
        prop_oneof![
            7 => 0_u64..=width,
            1 => Just(width.saturating_add(1)),
        ]
        .prop_map(move |r| (length, r))
    })
}

/// One generated request, tagged with the kernel that answers it.
#[derive(Clone, Debug)]
pub(super) enum Generated {
    /// A product request: operand lengths and a repeat count.
    Product {
        /// Lengths of the operands, in the caller's order.
        lengths: Vec<usize>,
        /// How many times the operand list is repeated.
        repeat: u64,
    },
    /// A combination request interpreted as `n choose r`.
    Combinations {
        /// Number of items to choose from.
        length: usize,
        /// Number of items chosen.
        r: u64,
    },
    /// The same request interpreted as a falling factorial.
    Permutations {
        /// Number of items to arrange.
        length: usize,
        /// Number of positions to fill.
        r: u64,
    },
    /// A combination request graded by the closed form rather than by a Pascal
    /// row, so a `u64`-wide `n` is reachable.
    ClosedForm {
        /// Number of items to choose from.
        length: usize,
        /// Number of items chosen, at most two.
        r: u64,
    },
}

impl Generated {
    /// Build the public request this generated value describes.
    fn request(&self) -> CardinalityRequest<'_> {
        match self {
            Self::Product { lengths, repeat } => CardinalityRequest::Product {
                lengths: lengths.as_slice(),
                repeat: *repeat,
            },
            Self::Combinations { length, r } | Self::ClosedForm { length, r } => {
                CardinalityRequest::Combinations {
                    length: *length,
                    r: *r,
                }
            }
            Self::Permutations { length, r } => CardinalityRequest::Permutations {
                length: *length,
                r: *r,
            },
        }
    }

    /// The cardinality the production kernel reports for this request.
    pub(super) fn cardinality(&self) -> Option<u64> {
        self.request().cardinality()
    }

    /// The ceiling this request kind is admitted under.
    pub(super) fn ceiling(&self) -> u64 {
        self.request().ceiling()
    }

    /// The oracle's answer for this request.
    ///
    /// The closed-form variant answers from the closed form even where the
    /// Pascal-triangle oracle also applies, because the point of that battery
    /// is that the two routes to `n choose r` agree.
    pub(super) fn expected(&self) -> Option<u64> {
        match self {
            Self::Product { lengths, repeat } => oracle::product(lengths, *repeat),
            Self::Combinations { length, r } => oracle::binomial(oracle::width_of(*length), *r),
            Self::ClosedForm { length, r } => {
                oracle::binomial_up_to_two(oracle::width_of(*length), *r)
            }
            Self::Permutations { length, r } => oracle::permutations(oracle::width_of(*length), *r),
        }
    }
}

/// Which battery a case drives.
#[derive(Clone, Copy, Debug)]
pub(super) enum Battery {
    /// The product kernel, over operand lengths and repeat counts.
    Product,
    /// The binomial kernel, over `n` up to [`COMBINATION_MAX_LENGTH`].
    Combination,
    /// The falling-factorial kernel, over `n` up to [`PERMUTATION_MAX_LENGTH`].
    Permutation,
    /// The binomial kernel against the closed form, over every `usize` and `r`
    /// up to two.
    ClosedForm,
}

impl Battery {
    /// The strategy this battery draws its requests from.
    pub(super) fn strategy(self) -> BoxedStrategy<Generated> {
        match self {
            Self::Product => product_request()
                .prop_map(|(lengths, repeat)| Generated::Product { lengths, repeat })
                .boxed(),
            Self::Combination => overshooting_request(COMBINATION_MAX_LENGTH)
                .prop_map(|(length, r)| Generated::Combinations { length, r })
                .boxed(),
            Self::Permutation => overshooting_request(PERMUTATION_MAX_LENGTH)
                .prop_map(|(length, r)| Generated::Permutations { length, r })
                .boxed(),
            Self::ClosedForm => (any::<usize>(), 0_u64..=2)
                .prop_map(|(length, r)| Generated::ClosedForm { length, r })
                .boxed(),
        }
    }

    /// This battery's name, for failure diagnostics.
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Product => "the product battery",
            Self::Combination => "the combination battery",
            Self::Permutation => "the permutation battery",
            Self::ClosedForm => "the closed-form battery",
        }
    }
}
