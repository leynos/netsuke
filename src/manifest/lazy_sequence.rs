//! Share a lazy-sequence fixture between manifest unit-test modules.
//!
//! Expansion tests need an iterator that records how far a consumer advanced.
//! Keeping the fixture here lets the laziness and resource-budget suites assert
//! early termination against one implementation rather than duplicating it.

use minijinja::value::{Kwargs, Value};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// Records how many values a lazily produced sequence has yielded.
#[derive(Clone, Debug)]
pub(crate) struct CountingSequence {
    /// Counts every value produced by an iterator derived from this sequence.
    pub(crate) produced: Arc<AtomicUsize>,
    /// Caps the number of values one iteration can produce.
    pub(crate) total: usize,
}

impl CountingSequence {
    /// Construct a sequence that will yield at most `total` values.
    pub(crate) fn new(total: usize) -> Self {
        Self {
            produced: Arc::new(AtomicUsize::new(0)),
            total,
        }
    }

    /// Return the shared handle that records produced values.
    pub(crate) fn produced(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.produced)
    }
}

/// Yields at most `total` values while recording each production.
struct CountingIter {
    /// Shares the production counter with the sequence.
    produced: Arc<AtomicUsize>,
    /// Holds the number of values still to yield.
    remaining: usize,
}

impl Iterator for CountingIter {
    type Item = Value;

    fn next(&mut self) -> Option<Value> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        let ordinal = self.produced.fetch_add(1, Ordering::SeqCst) + 1;
        Some(Value::from(ordinal))
    }
}

/// Convert a counting sequence into a `MiniJinja` iterable value.
pub(crate) fn counting_value(source: &CountingSequence) -> Value {
    Value::make_object_iterable(source.clone(), |state| {
        Box::new(CountingIter {
            produced: Arc::clone(&state.produced),
            remaining: state.total,
        })
    })
}

/// Install a `counting_range()` function that yields the supplied sequence.
pub(crate) fn add_counting_range(
    env: &mut minijinja::Environment<'static>,
    sequence: &CountingSequence,
) {
    let shared = sequence.clone();
    env.add_function("counting_range", move |_kwargs: Kwargs| -> Value {
        counting_value(&shared)
    });
}
