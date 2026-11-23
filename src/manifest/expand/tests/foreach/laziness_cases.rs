//! Proves `foreach` consumes its iterator lazily rather than collecting it.

use super::super::*;
use crate::manifest::ManifestBudgetLimits;
use crate::manifest::lazy_sequence::{CountingSequence, add_counting_range};
use anyhow::{Result, ensure};
use minijinja::Environment;
use std::sync::atomic::Ordering;
use test_support::fluent::normalize_fluent_isolates;

/// A `foreach` must stop at the cardinality check instead of collecting every
/// value first: the production counter records how far iteration actually ran,
/// so a materializing implementation produces the whole sequence.
#[test]
fn foreach_stops_consuming_before_the_cardinality_limit() -> Result<()> {
    const TOTAL: usize = 100_000;
    const CARDINALITY_LIMIT: usize = 2;
    let sequence = CountingSequence::new(TOTAL);
    let produced = sequence.produced();
    let mut env = Environment::new();
    add_counting_range(&mut env, &sequence);

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

    let error = expand_foreach_with_budget(&mut doc, &env, &budget, true)
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
