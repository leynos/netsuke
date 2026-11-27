//! Where a generated entry writes its control keys.

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
