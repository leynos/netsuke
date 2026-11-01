//! Bounded materialization for standard-library helpers.
//!
//! [RFC 0006 §6.8] and its table 3 require every helper that can be asked to
//! produce more output than the machine can hold to refuse the request *before*
//! it allocates anything, and to say exactly why. This module is that shared
//! contract: it owns the ceilings, the checked cardinality arithmetic, and the
//! typed refusals that the later standard-library helpers map into their own
//! diagnostic domains.
//!
//! # What is bounded
//!
//! The combinatorial ceilings are in scope:
//!
//! - [`OUTPUT_TUPLE_CEILING`], checked by [`CardinalityRequest::admit`] for
//!   `product` and `combinations`;
//! - [`PERMUTATION_TUPLE_CEILING`], the lower ceiling the same call applies to
//!   `permutations`, selected by the request kind.
//!
//! # Exactness and precedence
//!
//! A cardinality is computed from operand lengths alone, in checked arithmetic,
//! and is either exact or reported as [`CardinalityError::Overflow`]; it is
//! never an estimate, a saturating value, or a wrap. Overflow takes precedence
//! over the over-ceiling refusal, and an empty operand yields cardinality zero
//! even when other operands would overflow.
//!
//! # What is deliberately absent
//!
//! Refusals are typed, `Copy`, and carry only integers and enumerated kinds, so
//! constructing one allocates nothing. They hold no Fluent key, no
//! `netsuke::jinja::` diagnostic code, and no conversion into `minijinja::Error`
//! (D-03): roadmap 6.1.4 owns the localisation scaffolding, and each consuming
//! RFC assigns its own code to its own module. The English `Display` rendering
//! exists for developers and logs and must not be forwarded into a template
//! error.
//!
//! The module reads no environment, clock, filesystem, or global state, and
//! does not depend on `minijinja`: a consumer extracts lengths, byte counts,
//! and depth from its own values and maps the refusal into its own domain error.
//!
//! # Example
//!
//! ```
//! use netsuke::stdlib::CardinalityRequest;
//!
//! let admitted = CardinalityRequest::Product {
//!     lengths: &[100, 200],
//!     repeat: 1,
//! }
//! .admit()
//! .expect("20000 tuples fit under the ceiling");
//! assert_eq!(admitted.get(), 20_000);
//!
//! let refused = CardinalityRequest::Product {
//!     lengths: &[316, 317],
//!     repeat: 1,
//! }
//! .admit()
//! .expect_err("100172 tuples exceed the ceiling");
//! assert!(refused.to_string().contains("100172"));
//! ```
//!
//! [RFC 0006 §6.8]: ../../../docs/rfcs/rfc-0006-ansible-inspired-template-standard-library-expansion.md

mod cardinality;
mod kernel;

#[cfg(test)]
mod tests;

pub use self::cardinality::{AdmittedCount, CardinalityError, CardinalityRequest};

/// Largest number of tuples `product` and `combinations` may produce.
pub const OUTPUT_TUPLE_CEILING: u64 = 100_000;

/// Largest number of tuples `permutations` may produce.
///
/// Permutations grow faster than the other two shapes, so the table sets a
/// lower ceiling for them; [`CardinalityRequest::ceiling`] selects it from the
/// request kind, and no caller chooses a ceiling for itself.
pub const PERMUTATION_TUPLE_CEILING: u64 = 10_000;
