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
//! Six ceilings are in scope. Four are the families roadmap 6.1.3 names, and
//! the last two are the output-content bound six child RFCs require (D-12):
//!
//! - [`INPUT_LENGTH_CEILING`], checked by [`admit_input`] before parsing;
//! - [`NESTING_DEPTH_CEILING`], checked one container at a time by
//!   [`NestingDepth`];
//! - [`OUTPUT_TUPLE_CEILING`], checked by [`CardinalityRequest::admit`] for
//!   `product` and `combinations`;
//! - [`PERMUTATION_TUPLE_CEILING`], the lower ceiling the same call applies to
//!   `permutations`, selected by the request kind;
//! - [`MATCH_COUNT_CEILING`], counted as a match stream grows by
//!   [`collect_matches`] and [`RunningTotal`];
//! - [`OUTPUT_BYTES_CEILING`], counted the same way under
//!   [`StreamKind::OutputBytes`].
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
//! use netsuke::stdlib::{CardinalityRequest, admit_input};
//!
//! admit_input(b"a small manifest").expect("sixteen bytes are within the ceiling");
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
mod limits;
mod streams;

#[cfg(kani)]
mod verification;

#[cfg(test)]
mod tests;

pub use self::cardinality::{AdmittedCount, CardinalityError, CardinalityRequest};
pub use self::limits::{DepthExceeded, InputLengthExceeded, NestingDepth, admit_input};
pub use self::streams::{
    RunningTotal, StreamKind, StreamLimitExceeded, collect_matches, try_collect_matches,
};

/// Largest input, in bytes, that a parsing helper admits.
///
/// The ceiling is checked before the text is parsed, so a manifest cannot make
/// a parser scan an unreasonable amount of input. It is a compile-time
/// constant rather than configuration: RFC 0006 §6.8 defers configurability
/// until a consumer needs it.
pub const INPUT_LENGTH_CEILING: u64 = 8 * 1024 * 1024;

/// Largest number of enclosing containers a recursive helper admits.
///
/// A root value sits at depth zero, so depths one to this value are admitted
/// and the attempt to enter the next level is refused. See [`NestingDepth`].
pub const NESTING_DEPTH_CEILING: u32 = 128;

/// Largest number of tuples `product` and `combinations` may produce.
pub const OUTPUT_TUPLE_CEILING: u64 = 100_000;

/// Largest number of tuples `permutations` may produce.
///
/// Permutations grow faster than the other two shapes, so the table sets a
/// lower ceiling for them; [`CardinalityRequest::ceiling`] selects it from the
/// request kind, and no caller chooses a ceiling for itself.
pub const PERMUTATION_TUPLE_CEILING: u64 = 10_000;

/// Largest number of matches a materializing helper may collect.
pub const MATCH_COUNT_CEILING: u64 = 100_000;

/// Largest output content, in bytes, that a materializing helper may build.
///
/// A tuple count does not bound the content those tuples hold, so a helper
/// that materializes values runs this check after cardinality admission.
pub const OUTPUT_BYTES_CEILING: u64 = 8 * 1024 * 1024;
