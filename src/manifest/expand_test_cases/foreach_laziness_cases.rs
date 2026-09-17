//! Proves `foreach` consumes its iterator lazily rather than collecting it.

use super::*;
use crate::manifest::ManifestBudgetLimits;
use anyhow::{Result, ensure};
use minijinja::{
    Environment,
    value::{Kwargs, Value},
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use test_support::fluent::normalize_fluent_isolates;

/// Records how many values a lazily produced sequence has yielded.
#[derive(Clone, Debug)]
struct CountingSequence {
    /// Counts every value produced by an iterator derived from this sequence.
    produced: Arc<AtomicUsize>,
    /// Caps the number of values one iteration can produce.
    total: usize,
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
fn counting_value(source: &CountingSequence) -> Value {
    Value::make_object_iterable(source.clone(), |state| {
        Box::new(CountingIter {
            produced: Arc::clone(&state.produced),
            remaining: state.total,
        })
    })
}

/// A `foreach` must stop at the cardinality check instead of collecting every
/// value first: the production counter records how far iteration actually ran,
/// so a materialising implementation produces the whole sequence.
#[test]
fn foreach_stops_consuming_before_the_cardinality_limit() -> Result<()> {
    const TOTAL: usize = 100_000;
    const CARDINALITY_LIMIT: usize = 2;
    let sequence = CountingSequence {
        produced: Arc::new(AtomicUsize::new(0)),
        total: TOTAL,
    };
    let produced = Arc::clone(&sequence.produced);
    let mut env = Environment::new();
    env.add_function("counting_range", move |_kwargs: Kwargs| -> Value {
        counting_value(&sequence)
    });

    let yaml = concat!(
        "targets:\n",
        "  - name: '{{ item }}'\n",
        "    foreach: 'counting_range()'\n",
        "    command: echo ok\n",
    );
    let mut doc: ManifestValue = serde_saphyr::from_str(yaml)?;
    let limits = ManifestBudgetLimits {
        evaluation_fuel: 1_000_000,
        manifest_fuel: 1_000_000,
        foreach_cardinality: CARDINALITY_LIMIT,
        expanded_entries: TOTAL,
        ..ManifestBudgetLimits::default()
    };
    let budget = ManifestBudget::new(limits)?;

    let error = expand_foreach_with_budget(&mut doc, &env, &budget)
        .expect_err("the third value must exceed the per-foreach cardinality limit");
    let rendered = normalize_fluent_isolates(&format!("{error:#}"));
    ensure!(
        rendered.contains("resource budget exhausted") && rendered.contains("during foreach"),
        "the failure must report the per-foreach cardinality stage: {rendered}"
    );

    let yielded = produced.load(Ordering::SeqCst);
    ensure!(
        yielded == CARDINALITY_LIMIT + 1,
        "foreach must stop at the cardinality check after {expected} values, produced {yielded}",
        expected = CARDINALITY_LIMIT + 1
    );
    Ok(())
}
