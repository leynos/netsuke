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
//! [RFC 0006 §6.7]: ../../../docs/rfcs/0006-ansible-inspired-template-standard-library.md

use std::fmt;

use minijinja::value::ValueKind;
use thiserror::Error;

/// Location of the value refused while building a canonical snapshot.
///
/// Positions count snapshot edges rather than host traversal steps, so a
/// location stays meaningful even when a mapping key is refused before its
/// value is visited. The rendering is abbreviated in the middle once the path
/// is long, so a deeply nested input cannot produce a message longer than most
/// terminals; the number of omitted edges is stated, so an elided path is
/// never mistaken for an adjacent one.
///
/// A path is rendered once, when the refusal is constructed, and the refusal
/// then carries only that text. Keeping the type module-private is deliberate:
/// a variant field of a public enum is itself public, so a `SnapshotPath`
/// field would either widen the public surface or warn as a private type in a
/// public interface. Callers branch on [`CanonicalValueError::kind`]; the
/// location reaches them as prose.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SnapshotPath {
    /// Edge steps from the snapshot root to the refused value.
    steps: Vec<PathStep>,
}

/// One edge of a [`SnapshotPath`].
#[derive(Clone, Debug, Eq, PartialEq)]
enum PathStep {
    /// A member of a sequence, addressed by its index.
    Item(usize),
    /// A member of a string-keyed mapping, addressed by its key.
    Member(String),
}

impl SnapshotPath {
    /// Edge count beyond which the middle of a path is elided.
    const RENDER_LIMIT: usize = 8;
    /// Edge count kept at each end of an elided path.
    const RENDER_EDGE: usize = 3;

    /// Return the path of the root value.
    pub(super) const fn root() -> Self {
        Self { steps: Vec::new() }
    }

    /// Return a copy of this path extended by one sequence index.
    pub(super) fn with_item(&self, index: usize) -> Self {
        self.extended(PathStep::Item(index))
    }

    /// Return a copy of this path extended by one mapping key.
    pub(super) fn with_member(&self, key: &str) -> Self {
        self.extended(PathStep::Member(key.to_owned()))
    }

    /// Return a copy of this path with `step` appended.
    fn extended(&self, step: PathStep) -> Self {
        let mut steps = self.steps.clone();
        steps.push(step);
        Self { steps }
    }

    /// Render the path for a diagnostic message, eliding its middle if long.
    ///
    /// The elision fires only past [`Self::RENDER_LIMIT`] edges, which is more
    /// than twice [`Self::RENDER_EDGE`], so both ends of an abbreviated path are
    /// always present. The slice accessors still report absence rather than
    /// panicking, because a `Display` implementation must not abort a render.
    fn render_abbreviated(&self) -> String {
        let count = self.steps.len();
        if count <= Self::RENDER_LIMIT {
            return render_steps(&self.steps);
        }
        let head = render_steps(self.steps.get(..Self::RENDER_EDGE).unwrap_or_default());
        let tail = render_steps(
            self.steps
                .get(count - Self::RENDER_EDGE..)
                .unwrap_or_default(),
        );
        let elided = count - 2 * Self::RENDER_EDGE;
        format!("{head} … ({elided} more) … {tail}")
    }
}

impl fmt::Display for SnapshotPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render_abbreviated())
    }
}

impl PathStep {
    /// Render this step as a JSON-pointer-like suffix.
    fn render(&self) -> String {
        match self {
            Self::Item(index) => format!("[{index}]"),
            Self::Member(key) => format!("[{key:?}]"),
        }
    }
}

/// Render `steps` as a concatenation of JSON-pointer-like suffixes.
fn render_steps(steps: &[PathStep]) -> String {
    steps.iter().map(PathStep::render).collect()
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
/// `Clone`, and a hand-written implementation would have to invent semantics
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
    /// whose refusal is not a kind decision, and for
    /// [`Self::IncompatibleRepresentation`], where the reported kind is one the
    /// value failed to traverse as rather than a kind that was refused.
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
            | Self::Serialization { .. } => None,
        }
    }

    /// Construct an unsupported-kind refusal at `path`.
    pub(super) fn unsupported(kind: ValueKind, path: &SnapshotPath) -> Self {
        Self::UnsupportedKind {
            kind,
            path: path.to_string(),
        }
    }

    /// Construct a non-string mapping key refusal at `path`.
    pub(super) fn non_string_key(kind: ValueKind, path: &SnapshotPath) -> Self {
        Self::NonStringKey {
            kind,
            path: path.to_string(),
        }
    }

    /// Construct a non-finite number refusal at `path`.
    pub(super) fn non_finite(path: &SnapshotPath) -> Self {
        Self::NonFiniteNumber {
            path: path.to_string(),
        }
    }

    /// Construct an untraversable-host-value refusal at `path`.
    pub(super) fn invalid_traversal(path: &SnapshotPath) -> Self {
        Self::IncompatibleRepresentation {
            path: path.to_string(),
        }
    }
}
