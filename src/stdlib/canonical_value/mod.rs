//! Canonical value identity for `MiniJinja` values.
//!
//! [RFC 0006 §6.7] defines value equality for the ordered-set helpers as
//! RFC 8785 canonical-JSON byte-identity: two values are equal exactly when
//! their canonical keys are byte-identical. This module supplies that relation
//! as a foundation, so later helpers share one deterministic answer instead of
//! each deriving their own.
//!
//! # Supported domain
//!
//! The domain is JSON scalars, finite nested sequences, and string-keyed
//! mappings, including well-behaved custom `MiniJinja` objects that expose
//! those representations. Everything outside it is refused with a typed
//! [`CanonicalValueError`] *before* any coercion: undefined, invalid, bytes,
//! plain objects, iterables, the `now()` object, callables, non-string
//! mapping keys at any depth, and non-finite floats. Refusal is an error
//! rather than inequality, so a value compared with itself is an error, not
//! `false`; see [`canonical_eq`].
//!
//! # Two consequences of RFC 8785 worth stating
//!
//! RFC 8785 renders every number as an IEEE-754 binary64, so the key is not
//! injective over the integers: `2^53 + 1` and `2^53` share a key, as do `-0.0`
//! and `0.0`. That collapse is the specified behaviour, not a defect, and this
//! module never claims integer injectivity. Property names are ordered by raw
//! UTF-16 code unit with no normalization step, so canonically equivalent but
//! byte-distinct spellings stay distinct.
//!
//! # Host precondition and purity
//!
//! The adapter traverses a host value exactly once and never re-reads it, so
//! callers must supply finite, stable, acyclic values. `MiniJinja` exposes no
//! public object identity, so cycle detection is outside the enforceable
//! contract and is not claimed. The module performs no I/O and reads no
//! environmental input: no clock, locale, filesystem, process, or network
//! access, and no template registration. It adds no template function or
//! filter name.
//!
//! # Example
//!
//! ```
//! use minijinja::{context, value::Value};
//! use netsuke::stdlib::{canonical_eq, canonical_key};
//!
//! let first = Value::from_serialize(context! { a => 1, b => "x" });
//! let second = Value::from_serialize(context! { b => "x", a => 1 });
//! assert!(canonical_eq(&first, &second).expect("both maps are admitted"));
//!
//! // Member order does not matter; sequence order does.
//! assert!(!canonical_eq(&Value::from([1, 2]), &Value::from([2, 1])).expect("both arrays are admitted"));
//! canonical_key(&first).expect("the map is admitted");
//! ```
//!
//! [RFC 0006 §6.7]: ../../../docs/rfcs/0006-ansible-inspired-template-standard-library.md

mod admission;
mod error;
mod node;

#[cfg(test)]
mod tests;

use std::collections::HashSet;

use minijinja::value::Value;

use self::admission::snapshot;
pub use self::error::CanonicalValueError;
use self::node::to_canonical_bytes;

/// The canonical JSON key of an admitted value.
///
/// A key is opaque: the only way to obtain one is [`canonical_key`], so a key
/// that exists is always the canonical form of an admitted value. Equality and
/// hashing are byte-identity over the key's canonical JSON, which makes the
/// key usable as a set or map member without exposing its bytes.
///
/// # Example
///
/// ```
/// use minijinja::value::Value;
/// use netsuke::stdlib::canonical_key;
///
/// let key = canonical_key(&Value::from("text")).expect("a string is canonical");
/// assert_eq!(key, canonical_key(&Value::from("text")).expect("already admitted"));
/// ```
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CanonicalKey {
    /// The RFC 8785 canonical JSON of the admitted value.
    bytes: Vec<u8>,
}

impl CanonicalKey {
    /// Borrow the canonical JSON bytes this key was built from.
    ///
    /// Kept internal to the crate's tests: the public surface exposes a key
    /// only through equality and hashing, and the emitted bytes are compared
    /// against the canonicalizer directly rather than through any caller.
    #[cfg(test)]
    #[must_use]
    pub(super) fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Return the RFC 8785 canonical key of `value`.
///
/// The value is traversed once, into a private snapshot that preserves the
/// source representation of every admitted member, and that snapshot is
/// serialized. Values outside the supported domain are refused before any
/// serialization happens.
///
/// # Errors
///
/// Returns [`CanonicalValueError::UnsupportedKind`] for a value with no
/// canonical JSON form, [`CanonicalValueError::NonStringKey`] for a mapping
/// key that is not a string, [`CanonicalValueError::NonFiniteNumber`] for a
/// `NaN` or infinite float, and
/// [`CanonicalValueError::IncompatibleRepresentation`] for a host value that
/// cannot be traversed as the kind it reports.
///
/// # Example
///
/// ```
/// use minijinja::value::{Value, ValueKind};
/// use netsuke::stdlib::canonical_key;
///
/// let key = canonical_key(&Value::from(1)).expect("an integer is canonical");
/// assert!(key == canonical_key(&Value::from(1.0)).expect("a float is canonical"));
///
/// let refusal = canonical_key(&Value::UNDEFINED).expect_err("undefined is not canonical");
/// assert_eq!(refusal.kind(), Some(ValueKind::Undefined));
/// ```
pub fn canonical_key(value: &Value) -> Result<CanonicalKey, CanonicalValueError> {
    let snapshot = snapshot(value)?;
    Ok(CanonicalKey {
        bytes: to_canonical_bytes(&snapshot)?,
    })
}

/// Report whether two values have byte-identical canonical keys.
///
/// Both operands are admitted before any comparison, and the left operand is
/// admitted first, so a refusal names the left operand when both are invalid.
/// No `MiniJinja` equality fast path is taken: the answer derives from the
/// canonical keys alone.
///
/// # Errors
///
/// Returns the refusal of whichever operand is not in the supported domain,
/// including when the two operands are the same value. A refused value
/// compared with itself is an error, never `false`.
///
/// # Example
///
/// ```
/// use minijinja::value::Value;
/// use netsuke::stdlib::{CanonicalValueError, canonical_eq};
///
/// assert!(canonical_eq(&Value::from([1, 2]), &Value::from([1, 2])).expect("both admitted"));
/// assert!(!canonical_eq(&Value::from([1, 2]), &Value::from([2, 1])).expect("order matters"));
///
/// let bytes = Value::from_bytes(vec![1]);
/// let refusal = canonical_eq(&bytes, &bytes).expect_err("bytes are not canonical");
/// assert!(matches!(refusal, CanonicalValueError::UnsupportedKind { .. }));
/// ```
pub fn canonical_eq(left: &Value, right: &Value) -> Result<bool, CanonicalValueError> {
    let left_key = canonical_key(left)?;
    let right_key = canonical_key(right)?;
    Ok(left_key == right_key)
}

/// Deduplicate `values` canonically, returning the first representative of
/// each distinct key in first-appearance order.
///
/// Each value is admitted in input order, and a value is retained exactly when
/// its key has not been seen in an earlier position. The returned references
/// point into the caller's slice, so a retained value is the original value
/// rather than a reconstruction. The membership structure is private and its
/// iteration order never escapes: output order is decided solely by input
/// order.
///
/// # Errors
///
/// Returns the refusal of the earliest invalid input value. No partial result
/// is produced, and values after the refused one are not examined.
///
/// # Example
///
/// ```
/// use minijinja::value::Value;
/// use netsuke::stdlib::stable_deduplicate;
///
/// let input = [Value::from("a"), Value::from("b"), Value::from("a")];
/// let retained = stable_deduplicate(&input).expect("all values are canonical");
///
/// assert_eq!(retained.len(), 2);
/// assert!(std::ptr::eq(retained[0], &input[0]));
/// assert!(std::ptr::eq(retained[1], &input[1]));
/// ```
pub fn stable_deduplicate(values: &[Value]) -> Result<Vec<&Value>, CanonicalValueError> {
    let mut seen: HashSet<CanonicalKey> = HashSet::new();
    let mut retained: Vec<&Value> = Vec::new();
    for value in values {
        // Append-or-skip: the key is inserted first, and the value is retained
        // only when that insertion reports the key as new.
        let key = canonical_key(value)?;
        if seen.insert(key) {
            retained.push(value);
        }
    }
    Ok(retained)
}
