//! Typed refusals raised while admitting a `MiniJinja` value into the
//! canonical-JSON domain.
//!
//! [RFC 0006 §6.7] requires that a value with no canonical JSON form is
//! refused by a typed error naming its kind, rather than coerced through a
//! serializer or reported as merely unequal. The enum here is that refusal: it
//! says *what* was refused and *where* in the snapshot tree, and carries no
//! Fluent key, diagnostic code, or localised message, because roadmap 6.1.4
//! owns user-facing diagnostics and none is invented here.
//!
//! [`CanonicalValueError::kind`] exposes the offending `MiniJinja` kind as a
//! value rather than as prose, so a caller can branch on the refusal without
//! parsing a string.
//!
//! [RFC 0006 §6.7]: ../../../docs/rfcs/rfc-0006-ansible-inspired-template-standard-library-expansion.md

use std::fmt;

use minijinja::value::ValueKind;
use thiserror::Error;

use super::path::PathRef;

/// The bounded resource a canonical admission can exhaust.
///
/// Admission expands a shared host graph into one owned subtree per
/// occurrence, so the work is bounded by the *expanded* size of the input
/// rather than by its object count. Each variant names one ceiling that keeps
/// that expansion finite; a refusal reports which ceiling was reached and the
/// location that would have exceeded it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalResource {
    /// Distance below the snapshot root.
    Depth,
    /// Snapshot nodes, counted once per collection occurrence.
    Nodes,
    /// Bytes of admitted strings and member names.
    Bytes,
}

impl fmt::Display for CanonicalResource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Depth => "depth",
            Self::Nodes => "nodes",
            Self::Bytes => "bytes",
        })
    }
}

/// Reason a candidate value was refused admission to the canonical domain.
///
/// Each variant covers one refusal family, and each is decided before the
/// value reaches the canonicalizer. No variant reports a failed comparison:
/// for a refused value compared with itself the answer is an error, not
/// `false`.
///
/// The enum is neither `PartialEq` nor `Clone`: [`Self::Serialization`] holds a
/// [`serde_json::Error`], which implements none of `PartialEq`, `Eq`, or
/// `Clone`, and a handwritten implementation would have to invent semantics
/// for a foreign error type. Tests compare the rendered message or match the
/// variant instead.
///
/// # Examples
///
/// ```
/// use minijinja::value::{Value, ValueKind};
/// use netsuke::stdlib::{CanonicalValueError, canonical_key};
///
/// let error = canonical_key(&Value::from_bytes(vec![1])).unwrap_err();
/// assert_eq!(error.kind(), Some(ValueKind::Bytes));
/// ```
#[derive(Debug, Error)]
pub enum CanonicalValueError {
    /// The value's kind has no canonical JSON form.
    #[error("value of kind `{kind}` has no canonical JSON form at {path}")]
    UnsupportedKind {
        /// The kind that cannot participate in canonical equality.
        kind: ValueKind,
        /// Rendered location of the refused value in the snapshot tree.
        path: String,
    },
    /// A mapping key was not a string.
    ///
    /// `serde_json` coerces an integer or boolean key to a quoted string,
    /// which would let two distinct mappings collide, so the refusal happens
    /// here instead of being delegated to the serializer. The key is described
    /// by its kind, never by its text: key text is caller data.
    #[error("mapping key of kind `{kind}` is not a string at {path}")]
    NonStringKey {
        /// The kind of the refused key.
        kind: ValueKind,
        /// Rendered location of the mapping that holds the refused key.
        path: String,
    },
    /// The value is a number that has no JSON representation.
    #[error("non-finite floating-point value at {path}")]
    NonFiniteNumber {
        /// Rendered location of the refused number in the snapshot tree.
        path: String,
    },
    /// The host value could not be traversed as the kind it reports.
    ///
    /// A `MiniJinja` object whose enumeration is empty yields no members
    /// without reporting an error, and one whose reported length disagrees
    /// with what it yields cannot be snapshotted reproducibly. Both are
    /// reported here. A failure raised part-way through a host iteration is
    /// not observable through the public `MiniJinja` interface, and is covered
    /// by the documented finite, stable, acyclic host-value precondition
    /// rather than being claimed as detected.
    #[error("value at {path} cannot be traversed as the kind it reports")]
    IncompatibleRepresentation {
        /// Rendered location of the untraversable value in the snapshot tree.
        path: String,
    },
    /// The value would exceed a bound on the work admission may perform.
    ///
    /// A compact acyclic graph with shared descendants expands to many more
    /// occurrences than it holds objects, so admission charges each occurrence
    /// against shared depth, node, and byte ceilings. This variant reports the
    /// first ceiling a traversal would cross, before the subtree that crosses
    /// it is allocated. Values inside the supported domain under the default
    /// ceilings are unaffected.
    #[error("canonical admission exceeded the {resource} limit of {limit} at {path}")]
    ResourceLimitExceeded {
        /// The ceiling that was exceeded.
        resource: CanonicalResource,
        /// The configured ceiling, in the resource's own unit.
        limit: usize,
        /// Rendered location of the value that would have exceeded it.
        path: String,
    },
    /// The canonicalizer refused the snapshot.
    ///
    /// The snapshot tree admits nothing the canonicalizer rejects, and holds
    /// no non-string keys, so this variant reports a failure outside the
    /// admitted contract rather than an input the caller could have corrected.
    #[error("canonical serialization failed: {source}")]
    Serialization {
        /// The failure reported by `serde_json_canonicalizer`.
        #[source]
        source: serde_json::Error,
    },
}

impl CanonicalValueError {
    /// Return the kind of the value or key that was refused, where a single
    /// kind names it.
    ///
    /// Returns `None` for [`Self::NonFiniteNumber`] and [`Self::Serialization`],
    /// whose refusal is not a kind decision, for
    /// [`Self::IncompatibleRepresentation`], where the reported kind is one the
    /// value failed to traverse as rather than a kind that was refused, and for
    /// [`Self::ResourceLimitExceeded`], where the value was in the supported
    /// domain but the traversal needed more of a shared resource than the
    /// ceiling allows.
    ///
    /// # Examples
    ///
    /// ```
    /// use minijinja::value::{Value, ValueKind};
    /// use netsuke::stdlib::canonical_key;
    ///
    /// let error = canonical_key(&Value::from_function(|| 0)).unwrap_err();
    /// assert_eq!(error.kind(), Some(ValueKind::Plain));
    /// ```
    #[must_use]
    pub const fn kind(&self) -> Option<ValueKind> {
        match self {
            Self::UnsupportedKind { kind, .. } | Self::NonStringKey { kind, .. } => Some(*kind),
            Self::NonFiniteNumber { .. }
            | Self::IncompatibleRepresentation { .. }
            | Self::ResourceLimitExceeded { .. }
            | Self::Serialization { .. } => None,
        }
    }

    /// Construct an unsupported-kind refusal at `path`.
    pub(super) fn unsupported(kind: ValueKind, path: &PathRef<'_>) -> Self {
        Self::UnsupportedKind {
            kind,
            path: path.render(),
        }
    }

    /// Construct a non-string mapping key refusal at `path`.
    pub(super) fn non_string_key(kind: ValueKind, path: &PathRef<'_>) -> Self {
        Self::NonStringKey {
            kind,
            path: path.render(),
        }
    }

    /// Construct a non-finite number refusal at `path`.
    pub(super) fn non_finite(path: &PathRef<'_>) -> Self {
        Self::NonFiniteNumber {
            path: path.render(),
        }
    }

    /// Construct an untraversable-host-value refusal at `path`.
    pub(super) fn invalid_traversal(path: &PathRef<'_>) -> Self {
        Self::IncompatibleRepresentation {
            path: path.render(),
        }
    }

    /// Construct a resource-limit refusal at `path`.
    ///
    /// The ceiling is copied from the budget rather than read back from the
    /// host value, so the message names configured policy and never caller
    /// data beyond the location.
    pub(super) fn resource_limit(
        resource: CanonicalResource,
        limit: usize,
        path: &PathRef<'_>,
    ) -> Self {
        Self::ResourceLimitExceeded {
            resource,
            limit,
            path: path.render(),
        }
    }
}
