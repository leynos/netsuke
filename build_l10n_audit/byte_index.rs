//! A byte position within the source a scanner walks.
//!
//! Split from `scanner.rs` to keep that module within the repository's line
//! limit. The type is deliberately tiny: it exists so a position cannot be
//! confused with a count, both being `usize` underneath.

/// A byte offset into a scanner's source.
///
/// Positions and counts are both `usize` underneath, and the scanner passes
/// them side by side — a raw string literal's opening index next to its run of
/// hashes. Naming the position separately keeps the two from being swapped.
#[derive(Clone, Copy)]
pub(crate) struct ByteIndex(usize);

impl ByteIndex {
    /// The start of the parsed body.
    pub(crate) const START: Self = Self(0);

    /// The position at byte `offset`.
    pub(crate) const fn from_offset(offset: usize) -> Self {
        Self(offset)
    }

    /// The wrapped offset as a `usize`.
    pub(crate) const fn get(self) -> usize {
        self.0
    }

    /// The position `delta` bytes further along.
    pub(crate) const fn advance(self, delta: usize) -> Self {
        Self(self.0 + delta)
    }

    /// The position `delta` bytes earlier, or `None` when that would underflow.
    pub(crate) const fn retreat(self, delta: usize) -> Option<Self> {
        match self.0.checked_sub(delta) {
            Some(offset) => Some(Self(offset)),
            None => None,
        }
    }
}
