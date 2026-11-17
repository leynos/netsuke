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

/// Location of the value refused while building a canonical snapshot.
///
/// Positions count snapshot edges rather than host traversal steps, so a
/// location stays meaningful even when a mapping key is refused before its
/// value is visited. The rendering is abbreviated in the middle once the path
/// has more than a handful of edges, so a deeply nested input cannot produce a
/// message that repeats the whole path; the number of omitted edges is stated,
/// so an elided path is never mistaken for an adjacent one. Elision bounds the
/// edge count, not the bytes: the member names a path retains are names
/// admission already charged against its byte ceiling.
///
/// A path is rendered once, when the refusal is constructed, and the refusal
/// then carries only that text. Keeping the type module-private is deliberate:
/// a variant field of a public enum is itself public, so a `SnapshotPath`
/// field would either widen the public surface or warn as a private type in a
/// public interface. Callers branch on [`CanonicalValueError::kind`]; the
/// location reaches them as prose.
///
/// Traversal does not carry this type. It carries [`PathRef`], a borrowed
/// chain of edges, and builds this owned path only when a refusal is
/// constructed, because extending an owned path once per child would copy
/// every ancestor member name once per descendant.
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
    /// Location rendered in place of the empty path of the snapshot root.
    const ROOT_MARKER: &str = "<root>";

    /// Wrap already-ordered steps as a path.
    const fn from_steps(steps: Vec<PathStep>) -> Self {
        Self { steps }
    }

    /// Render the path for a diagnostic message, eliding its middle if long.
    ///
    /// An empty path names the root, which is the most common refusal site and
    /// the one every other path is written relative to. It therefore renders as
    /// [`Self::ROOT_MARKER`] rather than as nothing: every refusal message
    /// places this rendering after a preposition, so an empty one would leave
    /// the message trailing off exactly where the caller needs the location.
    ///
    /// The elision fires only past [`Self::RENDER_LIMIT`] edges, which is more
    /// than twice [`Self::RENDER_EDGE`], so both ends of an abbreviated path are
    /// always present. The slice accessors still report absence rather than
    /// panicking, because a `Display` implementation must not abort a render.
    fn render_abbreviated(&self) -> String {
        let count = self.steps.len();
        if count == 0 {
            return Self::ROOT_MARKER.to_owned();
        }
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

/// A borrowed snapshot path, extended once per child during traversal.
///
/// An owned path would copy every ancestor member name once per descendant, so
/// the work would scale with the product of the node ceiling and the byte
/// ceiling rather than with their sum: a wide collection beneath long member
/// names would copy megabytes for every item while every individual charge
/// stayed inside its own ceiling. A refusal is rare, so the borrowed chain is
/// walked and copied only when a refusal is constructed.
#[derive(Clone, Copy, Debug)]
pub(super) enum PathRef<'a> {
    /// The path of the snapshot root.
    Root,
    /// A sequence member of `parent`, addressed by its index.
    Item {
        /// Path of the sequence this item belongs to.
        parent: &'a PathRef<'a>,
        /// Index of the item in that sequence.
        index: usize,
    },
    /// A mapping member of `parent`, addressed by its key.
    Member {
        /// Path of the mapping this member belongs to.
        parent: &'a PathRef<'a>,
        /// Name of the member in that mapping.
        key: &'a str,
    },
}

impl<'a> PathRef<'a> {
    /// Return the path of the root value.
    pub(super) const fn root() -> Self {
        Self::Root
    }

    /// Return this path extended by one sequence index.
    pub(super) const fn with_item<'b>(&'b self, index: usize) -> PathRef<'b>
    where
        'a: 'b,
    {
        PathRef::Item {
            parent: self,
            index,
        }
    }

    /// Return this path extended by one mapping key.
    pub(super) const fn with_member<'b>(&'b self, key: &'b str) -> PathRef<'b>
    where
        'a: 'b,
    {
        PathRef::Member { parent: self, key }
    }

    /// Render this path for a refusal message.
    ///
    /// This is the only place the borrowed chain is copied, so the traversal
    /// pays for a path only when it has something to report.
    pub(super) fn render(&self) -> String {
        self.to_snapshot_path().to_string()
    }

    /// Materialize the owned path, walking from the refused value to the root.
    fn to_snapshot_path(&self) -> SnapshotPath {
        let mut reversed = Vec::new();
        let mut cursor: &PathRef<'_> = self;
        loop {
            match cursor {
                Self::Root => break,
                Self::Item { parent, index } => {
                    reversed.push(PathStep::Item(*index));
                    cursor = parent;
                }
                Self::Member { parent, key } => {
                    reversed.push(PathStep::Member((*key).to_owned()));
                    cursor = parent;
                }
            }
        }
        reversed.reverse();
        SnapshotPath::from_steps(reversed)
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
