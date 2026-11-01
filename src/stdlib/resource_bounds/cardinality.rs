//! Checked combinatorial cardinality and its admission token.
//!
//! A combinatorial helper is asked to build one tuple per combination of its
//! operands, so the number of tuples is a property of the operand *lengths*
//! alone. Describing the request that way lets this module compute the exact
//! count — and refuse an unreasonable one — without the operands, the template
//! values, or any output existing yet.
//!
//! Each request kind fixes its own ceiling, and the only way to obtain an
//! [`AdmittedCount`] is to pass through [`CardinalityRequest::admit`], so a
//! count that exists has been checked.

use thiserror::Error;

use super::{OUTPUT_TUPLE_CEILING, PERMUTATION_TUPLE_CEILING, kernel};

/// A combinatorial request described by operand lengths, never by output.
///
/// The variants name the three shapes the standard library exposes. Which
/// ceiling applies is decided by the variant rather than supplied by the
/// caller, so the defect of checking permutations against the product ceiling
/// cannot be expressed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CardinalityRequest<'a> {
    /// A Cartesian product over `lengths`, raised to `repeat`.
    Product {
        /// Lengths of the operands, in the caller's order.
        lengths: &'a [usize],
        /// How many times the operand list is repeated.
        repeat: u64,
    },
    /// Unordered selections of `r` items from `length`.
    Combinations {
        /// Number of items to choose from.
        length: usize,
        /// Number of items chosen.
        r: u64,
    },
    /// Ordered selections of `r` items from `length`.
    Permutations {
        /// Number of items to arrange.
        length: usize,
        /// Number of positions to fill.
        r: u64,
    },
}

impl CardinalityRequest<'_> {
    /// Return the exact number of tuples this request would produce.
    ///
    /// The answer is `None` exactly when the mathematical cardinality exceeds
    /// `u64::MAX`. Every other case is the exact value, including zero for an
    /// empty operand and one for an empty selection.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::stdlib::CardinalityRequest;
    ///
    /// let chosen = CardinalityRequest::Combinations { length: 5, r: 2 };
    /// assert_eq!(chosen.cardinality(), Some(10));
    ///
    /// let empty = CardinalityRequest::Product { lengths: &[], repeat: 1 };
    /// assert_eq!(empty.cardinality(), Some(1), "one empty tuple");
    ///
    /// let wide = CardinalityRequest::Combinations { length: usize::MAX, r: 40 };
    /// assert_eq!(wide.cardinality(), None, "above u64::MAX");
    /// ```
    #[must_use]
    pub fn cardinality(&self) -> Option<u64> {
        match *self {
            Self::Product { lengths, repeat } => kernel::checked_product(lengths, repeat),
            Self::Combinations { length, r } => binomial_of(length, r),
            Self::Permutations { length, r } => falling_factorial_of(length, r),
        }
    }

    /// Return the ceiling this request kind is checked against.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::stdlib::{CardinalityRequest, OUTPUT_TUPLE_CEILING, PERMUTATION_TUPLE_CEILING};
    ///
    /// let product = CardinalityRequest::Product { lengths: &[2], repeat: 1 };
    /// assert_eq!(product.ceiling(), OUTPUT_TUPLE_CEILING);
    ///
    /// let permutations = CardinalityRequest::Permutations { length: 2, r: 2 };
    /// assert_eq!(permutations.ceiling(), PERMUTATION_TUPLE_CEILING);
    /// ```
    #[must_use]
    pub const fn ceiling(&self) -> u64 {
        match self {
            Self::Permutations { .. } => PERMUTATION_TUPLE_CEILING,
            Self::Product { .. } | Self::Combinations { .. } => OUTPUT_TUPLE_CEILING,
        }
    }

    /// Admit this request, or refuse it before any output exists.
    ///
    /// # Errors
    ///
    /// Returns [`CardinalityError::Overflow`] when the exact cardinality is
    /// above `u64::MAX`, and [`CardinalityError::Exceeded`] when it is known
    /// but above this request kind's ceiling. Overflow is reported in
    /// preference to an over-ceiling refusal.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::stdlib::{CardinalityError, CardinalityRequest};
    ///
    /// let request = CardinalityRequest::Product { lengths: &[100, 200], repeat: 1 };
    /// let admitted = request.admit().expect("20000 tuples fit under the ceiling");
    /// assert_eq!(admitted.get(), 20_000);
    ///
    /// let refused = CardinalityRequest::Permutations { length: 8, r: 8 }
    ///     .admit()
    ///     .expect_err("40320 permutations exceed the lower ceiling");
    /// assert!(matches!(
    ///     refused,
    ///     CardinalityError::Exceeded {
    ///         cardinality: 40_320,
    ///         ceiling: 10_000,
    ///     }
    /// ));
    /// ```
    pub fn admit(&self) -> Result<AdmittedCount, CardinalityError> {
        let cardinality = kernel::classify(self.cardinality(), self.ceiling())?;
        Ok(AdmittedCount::new(cardinality))
    }
}

/// Evaluate a binomial whose `n` arrives as a collection length.
///
/// A length wider than `u64` cannot be represented by the kernel. Choosing
/// nothing from any set is still exactly one selection, so `r == 0` answers
/// before the conversion; every other selection from a set that large is
/// certainly above `u64::MAX`.
fn binomial_of(length: usize, r: u64) -> Option<u64> {
    match u64::try_from(length) {
        Ok(n) => kernel::checked_binomial(n, r),
        Err(_) if r == 0 => Some(1),
        Err(_) => None,
    }
}

/// Evaluate a falling factorial whose `n` arrives as a collection length.
///
/// The reasoning is the same as [`binomial_of`]: an empty arrangement of any
/// set is the single empty tuple, and any non-empty arrangement of a set wider
/// than `u64` is above `u64::MAX`.
fn falling_factorial_of(length: usize, r: u64) -> Option<u64> {
    match u64::try_from(length) {
        Ok(n) => kernel::checked_falling_factorial(n, r),
        Err(_) if r == 0 => Some(1),
        Err(_) => None,
    }
}

/// An approved tuple count, at most the ceiling for its request kind.
///
/// The field is private and the only constructor is
/// [`CardinalityRequest::admit`], so a count that exists has been checked.
/// It is a tuple count, not a memory budget: RFC 0015 §5.8 still requires an
/// output-content check, because a hundred thousand tuples can each hold a
/// hundred thousand elements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedCount(usize);

impl AdmittedCount {
    /// Return the approved count.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::stdlib::CardinalityRequest;
    ///
    /// let admitted = CardinalityRequest::Combinations { length: 5, r: 2 }
    ///     .admit()
    ///     .expect("ten selections fit under the ceiling");
    /// assert_eq!(admitted.get(), 10);
    /// ```
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }

    /// Wrap a value the caller has already admitted.
    ///
    /// Every ceiling is at most [`OUTPUT_TUPLE_CEILING`], which fits in a
    /// 32-bit `usize`, and no supported target is narrower than that. The
    /// fallback keeps the constructor total without a panic in production
    /// code; it cannot be reached.
    #[must_use]
    fn new(value: u64) -> Self {
        Self(usize::try_from(value).unwrap_or(usize::MAX))
    }
}

/// Why a combinatorial request was refused.
///
/// Both variants are refusals rather than failures: the request is well formed
/// and the answer is known, but it is not admissible. Consumers map each
/// variant to its own diagnostic code, which is why they are separate rather
/// than one `Exceeded` carrying a flag.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum CardinalityError {
    /// The exact cardinality is above the ceiling for the request kind.
    #[error(
        "combinatorial cardinality {cardinality} exceeds the ceiling of {ceiling} output tuples"
    )]
    Exceeded {
        /// The exact number of tuples the request would produce.
        cardinality: u64,
        /// The ceiling fixed by the request kind.
        ceiling: u64,
    },
    /// The cardinality is not representable in `u64`.
    #[error(
        "combinatorial cardinality exceeds {}; the ceiling is {ceiling} output tuples",
        u64::MAX
    )]
    Overflow {
        /// The ceiling fixed by the request kind.
        ceiling: u64,
    },
}
