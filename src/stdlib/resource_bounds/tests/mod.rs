//! Shared fixtures and the batteries for bounded materialization.
//!
//! Every case in this subtree drives the public surface — the six ceilings,
//! `CardinalityRequest`, `admit_input`, `NestingDepth`, `RunningTotal`, and the
//! two match collectors — so a refusal is the typed value a consumer receives
//! and an admitted count is one the public API produced. The kernel's
//! arithmetic is reached only where a battery names it explicitly, which is the
//! `admission` battery's `classify` property and the cap witnesses in
//! `cardinality`.
//!
//! The batteries split by the kind of judgement each makes. `oracle` owns the
//! independent `u128` expectations that no production path computes;
//! `properties` owns the generated cases and their class-coverage tripwires;
//! `cardinality` owns the named arithmetic witnesses; `admission` owns the
//! boundary table and the argument that refusal allocation cannot happen;
//! `limits` owns depth, input-length, and stream boundaries; `diagnostics` owns
//! the snapshots that pin each refusal's rendered message. The fixture below
//! lives here rather than beside its first caller so that a battery cannot
//! quietly drift onto a private copy of it.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

mod admission;
mod cardinality;
mod diagnostics;
mod limits;
mod oracle;
mod properties;

/// A stream that records how many times it was pulled.
///
/// The count is taken at the point of the pull rather than inferred from how
/// many items were retained, because "the collector stops one item past the
/// ceiling" is a claim about what it read, not about what it kept. The counter
/// is shared rather than returned with the items, so a case can consume the
/// stream and still ask how much of it was consumed.
#[derive(Debug)]
pub(super) struct CountingStream<I> {
    /// The wrapped stream, in whatever shape the case needs.
    inner: I,
    /// Pulls observed so far, shared with the case that owns the wrapper.
    pulls: Arc<AtomicUsize>,
}

impl<I> CountingStream<I> {
    /// Wrap `inner`, starting its pull count at zero.
    #[must_use]
    pub(super) fn new(inner: I) -> Self {
        Self {
            inner,
            pulls: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Return how many times the stream has been pulled.
    ///
    /// A finite stream's count includes the pull that reported exhaustion, so a
    /// case asserting an exact count on a finite stream must allow for that
    /// extra pull. An infinite stream never pays it, which is what makes the
    /// ceiling witness exact.
    #[must_use]
    pub(super) fn pulls(&self) -> usize {
        self.pulls.load(Ordering::SeqCst)
    }
}

impl<I: Iterator> Iterator for CountingStream<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.pulls.fetch_add(1, Ordering::SeqCst);
        self.inner.next()
    }
}
