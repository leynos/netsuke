//! Where a generated entry writes its control keys.
//!
//! Each control key takes its own slot. `ControlLayout` supplies the slot,
//! `ControlOrder` decides which key takes the earlier one, and
//! `ControlAdjacency` decides whether the two slots coincide. Keeping the
//! three apart means the layout can vary without touching the entry model.

use proptest::prelude::*;
use proptest::sample::Index;

/// Where the control block sits in the authored key sequence.
#[derive(Clone, Copy, Debug)]
pub(super) enum ControlLayout {
    /// A drawn position among the authored keys.
    Drawn(Index),
    /// Forced to the end, for the coverage test's negative control.
    Last,
}

/// Which control key an entry with both writes first.
///
/// `swap_remove` corrupts order by moving the last key into a vacated slot,
/// so the two removal sites interact whenever an entry carries both. Drawing
/// the order means both interleavings are exercised rather than one.
#[derive(Clone, Copy, Debug)]
pub(in crate::manifest::expand::property_tests) enum ControlOrder {
    /// `foreach` then `when`.
    ForeachFirst,
    /// `when` then `foreach`.
    WhenFirst,
}

impl ControlOrder {
    /// Generate one order.
    pub(super) fn strategy() -> impl Strategy<Value = Self> {
        prop_oneof![Just(Self::ForeachFirst), Just(Self::WhenFirst)]
    }
}

/// Whether an entry's two control keys are written next to each other.
#[derive(Clone, Copy, Debug)]
pub(in crate::manifest::expand::property_tests) enum ControlAdjacency {
    /// The two control keys are adjacent, so removing both leaves the authored
    /// fields' relative order unchanged.
    Adjacent,
    /// Authored fields sit between the two control keys, so each removal moves
    /// a different key and the two sites' interactions become visible.
    Separated,
}

impl ControlAdjacency {
    /// Generate one adjacency choice.
    pub(super) fn strategy() -> impl Strategy<Value = Self> {
        prop_oneof![Just(Self::Adjacent), Just(Self::Separated)]
    }
}

impl ControlLayout {
    /// A drawn position among the authored keys.
    pub(super) const fn drawn(index: Index) -> Self {
        Self::Drawn(index)
    }

    /// Resolve the position the block is inserted at.
    pub(super) fn position(self, base_len: usize) -> usize {
        match self {
            Self::Drawn(index) => index.index(base_len + 1),
            Self::Last => base_len,
        }
    }
}
