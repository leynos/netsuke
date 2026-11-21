//! Snapshot locations recorded in canonical-admission refusals.
//!
//! Admission walks a value with [`PathRef`], a borrowed chain of snapshot edges
//! extended once per child, and materializes the owned [`SnapshotPath`] only
//! when a refusal is constructed. The indirection is a cost decision: extending
//! an owned path once per child would copy every ancestor member name for every
//! descendant, so path bookkeeping would grow with the product of the node and
//! byte ceilings rather than with the work each ceiling bounds.
//!
//! Rendering abbreviates the middle of a long path and states how many edges it
//! omitted, so a deeply nested input cannot produce a message that repeats the
//! whole path.

use std::fmt;

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
/// public interface. Callers branch on
/// [`CanonicalValueError::kind`](super::error::CanonicalValueError::kind); the
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
#[derive(Debug, Eq, PartialEq)]
enum PathStep {
    /// A member of a sequence, addressed by its index.
    Item(usize),
    /// A member of a string-keyed mapping, addressed by its key.
    Member(String),
}

impl Clone for PathStep {
    /// Clone one edge, counting the clone under test instrumentation.
    ///
    /// Cloning a [`PathStep`] is the operation this module exists to avoid
    /// during traversal: a `Member` step owns its name, so cloning one per
    /// descendant copies every ancestor name once per descendant. Counting the
    /// clone makes that regression observable, which no assertion over the
    /// admitted key or the pull count can be. The counter is compiled only
    /// under `cfg(test)`, so no production cost is added.
    fn clone(&self) -> Self {
        #[cfg(test)]
        STEP_CLONES.with(|clones| clones.set(clones.get().saturating_add(1)));
        match self {
            Self::Item(index) => Self::Item(*index),
            Self::Member(key) => Self::Member(key.clone()),
        }
    }
}

#[cfg(test)]
thread_local! {
    /// Path steps cloned on this thread since the last reset.
    static STEP_CLONES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Return how many [`PathStep`] clones happened on this thread, resetting it.
///
/// Thread-local rather than global because each test runs in its own thread
/// under nextest, and a shared counter would let an unrelated case's clones
/// decide this one's verdict. Exposed only to the module's own test batteries.
#[cfg(test)]
pub(super) fn take_step_clones() -> usize {
    STEP_CLONES.with(|clones| clones.replace(0))
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
        parent: &'a Self,
        /// Index of the item in that sequence.
        index: usize,
    },
    /// A mapping member of `parent`, addressed by its key.
    Member {
        /// Path of the mapping this member belongs to.
        parent: &'a Self,
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
    fn to_snapshot_path(self) -> SnapshotPath {
        let mut reversed = Vec::new();
        let mut cursor: &Self = &self;
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
