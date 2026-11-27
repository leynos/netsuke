//! `OBL-GEN`: each generator still reaches every class its properties need.

use super::strategy::{
    document_spec_strategy, entry_spec_strategy, foreach_entry_spec_strategy,
    static_condition_entry_spec_strategy, static_document_spec_strategy,
};
use classifiers::{entry_classes, forced_control_classes, injection_classes, static_classes};
use document_classes::document_classes;
use proptest::strategy::Strategy;

mod classifiers;
mod document_classes;
use proptest::test_runner::{Config, RngSeed, TestRunner};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

/// Cases each classifier draws.
const COVERAGE_CASES: u32 = 512;

/// Seed for the classifiers' runner.
const COVERAGE_SEED: u64 = 0x432;

/// Share of draws each class must reach, in percent.
const MINIMUM_CLASS_PERCENT: usize = 5;

/// One classification tally, counting draws rather than occurrences.
type Tally = BTreeMap<&'static str, usize>;

/// The classes one draw belongs to.
type ClassSet = BTreeSet<&'static str>;

/// Record that one draw belongs to `class`.
fn count(classes: &mut ClassSet, class: &'static str) {
    classes.insert(class);
}

/// Count the classes of a fixed-seed sample of one strategy.
fn tally<S: Strategy>(
    strategy: &S,
    classify: impl Fn(&S::Value, &mut ClassSet) -> bool,
) -> Result<(Tally, usize), String> {
    let mut runner = TestRunner::new(Config {
        cases: COVERAGE_CASES,
        rng_seed: RngSeed::Fixed(COVERAGE_SEED),
        ..Config::default()
    });
    let counts = RefCell::new(Tally::new());
    let in_scope = RefCell::new(0usize);
    runner
        .run(strategy, |value| {
            let mut classes = ClassSet::new();
            if !classify(&value, &mut classes) {
                return Ok(());
            }
            *in_scope.borrow_mut() += 1;
            let mut tally = counts.borrow_mut();
            for class in classes {
                *tally.entry(class).or_insert(0) += 1;
            }
            Ok(())
        })
        .map_err(|error| error.to_string())?;
    Ok((counts.into_inner(), in_scope.into_inner()))
}

/// Fail when any required class reaches less than the floor.
fn check_floors(tally: &Tally, required: &[&'static str], total: usize) -> Result<(), String> {
    // An empty denominator would make every floor zero and pass a sample that
    // never exercised the classes at all.
    if total == 0 {
        return Err(format!(
            "the sample placed no draw in scope, so no class can be checked: {tally:?}"
        ));
    }
    // Round up: 5% of 512 is 25.6, so a floor of 25 would sit below the
    // stated share and admit a class rarer than the contract allows.
    let floor = total.saturating_mul(MINIMUM_CLASS_PERCENT).div_ceil(100);
    let unreached: Vec<String> = required
        .iter()
        .filter(|class| tally.get(**class).copied().unwrap_or(0) < floor)
        .map(|class| {
            format!(
                "{class} reached {} of {total} draws",
                tally.get(*class).copied().unwrap_or(0)
            )
        })
        .collect();
    if unreached.is_empty() {
        return Ok(());
    }
    Err(format!(
        "classes below {MINIMUM_CLASS_PERCENT}% of {total} draws: {}; all counts: {tally:?}",
        unreached.join("; ")
    ))
}

/// Write one tally to standard output for a `--no-capture` run.
fn report(label: &str, tally: &Tally) {
    use std::io::Write as _;
    writeln!(std::io::stdout(), "{label} classes: {tally:?}").ok();
}

#[test]
fn entry_generator_reaches_every_material_class() -> Result<(), String> {
    let (tally, draws) = tally(&entry_spec_strategy(), entry_classes)?;
    report("entry", &tally);
    check_floors(&tally, ENTRY_CLASSES, draws)
}

#[test]
fn foreach_generator_reaches_every_injection_class() -> Result<(), String> {
    let (tally, draws) = tally(&foreach_entry_spec_strategy(), injection_classes)?;
    report("inject", &tally);
    check_floors(&tally, INJECT_CLASSES, draws)
}

/// Classify the strategy the `when`-order property draws from.
#[test]
fn static_condition_generator_reaches_every_verdict_class() -> Result<(), String> {
    let (tally, draws) = tally(&static_condition_entry_spec_strategy(), static_classes)?;
    report("static-condition", &tally);
    check_floors(&tally, STATIC_CLASSES, draws)
}

/// Classify the strategy the static-survival property draws from.
#[test]
fn static_document_generator_reaches_every_verdict_class() -> Result<(), String> {
    let (tally, draws) = tally(&static_document_spec_strategy(), document_classes)?;
    report("static-document", &tally);
    check_floors(&tally, STATIC_DOCUMENT_CLASSES, draws)
}

/// Classify the transformed entry the foreach order property actually expands.
#[test]
fn foreach_order_generator_reaches_every_control_class() -> Result<(), String> {
    let transformed = foreach_entry_spec_strategy().prop_map(|spec| super::strategy::EntrySpec {
        section: spec.section,
        body: spec.body.without_condition(),
    });
    let (tally, draws) = tally(&transformed, entry_classes)?;
    report("foreach-order", &tally);
    check_floors(&tally, FOREACH_ORDER_CLASSES, draws)
}

#[test]
fn document_generator_reaches_every_section_class() -> Result<(), String> {
    let (tally, draws) = tally(&document_spec_strategy(), document_classes)?;
    report("document", &tally);
    check_floors(&tally, DOCUMENT_CLASSES, draws)
}

/// Verify the floor check reports a class the sample never reaches.
#[test]
fn the_threshold_check_reports_an_unreached_class() -> Result<(), String> {
    let (observed, draws) = tally(&foreach_entry_spec_strategy(), forced_control_classes)?;
    let unreached = "control-two-or-more-following";
    let failure = check_floors(
        &observed,
        &["control-last", "control-one-following", unreached],
        draws,
    )
    .expect_err("a sample with the forced layout must fail the floor check");
    if !failure.contains(unreached) {
        return Err(format!(
            "the floor check failed but did not name `{unreached}`: {failure}"
        ));
    }
    Ok(())
}

/// Classes the general entry generator must reach.
pub(super) const ENTRY_CLASSES: &[&str] = &[
    "targets",
    "actions",
    "static",
    "foreach",
    "vars",
    "no-vars",
    "condition",
    "no-condition",
    "condition-expression",
    "condition-template",
    "condition-reads-vars",
    "condition-reads-index",
    "condition-reads-item",
    "control-last",
    "control-one-following",
    "control-two-or-more-following",
    "control-single",
    "control-both-foreach-first",
    "control-both-when-first",
    "control-same-slot",
    "control-two-slots",
    "foreach-empty",
    "foreach-non-empty",
    "foreach-array",
    "foreach-string",
    "value-null",
    "value-bool",
    "value-number",
    "value-string",
    "value-array",
    "value-object",
    "value-nested",
    "value-jinja-string",
];

/// Classes the foreach generator must reach for the injection obligations.
pub(super) const INJECT_CLASSES: &[&str] = &[
    "foreach-array",
    "foreach-string",
    "control-last",
    "control-one-following",
    "control-two-or-more-following",
    "control-same-slot",
    "control-two-slots",
    "inject-empty",
    "inject-all-included",
    "inject-all-excluded",
    "inject-partial",
    "inject-gap",
    "item-null",
    "item-bool",
    "item-number",
    "item-string",
    "item-array",
    "item-object",
    "item-null-string",
    "user-item-collision",
    "user-index-collision",
];

/// Classes the static-condition generator must reach.
pub(super) const STATIC_CLASSES: &[&str] = &[
    "control-last",
    "control-one-following",
    "control-two-or-more-following",
    "shape-literal",
    "shape-flag",
    "shape-not-flag",
    "shape-count",
    "shape-var-cmp",
    "targets-held-expression",
    "targets-held-template",
    "targets-failed-expression",
    "targets-failed-template",
    "actions-held-expression",
    "actions-held-template",
    "actions-failed-expression",
    "actions-failed-template",
    "reads-vars",
];

/// Classes the document generator must reach.
pub(super) const DOCUMENT_CLASSES: &[&str] = &[
    "targets-entries",
    "actions-entries",
    "document-static",
    "document-foreach",
    "targets-entry-held",
    "targets-entry-failed",
    "targets-entry-absent",
    "targets-entry-unevaluated",
    "actions-entry-held",
    "actions-entry-failed",
    "actions-entry-absent",
    "actions-entry-unevaluated",
];

/// Classes the transformed foreach-order input must reach.
pub(super) const FOREACH_ORDER_CLASSES: &[&str] = &[
    "control-last",
    "control-one-following",
    "control-two-or-more-following",
    "foreach-array",
    "foreach-string",
];

/// Classes the static-document strategy must reach.
pub(super) const STATIC_DOCUMENT_CLASSES: &[&str] = &[
    "targets-entries",
    "actions-entries",
    "document-static",
    "targets-entry-held",
    "targets-entry-failed",
    "targets-entry-absent",
    "targets-held-expression",
    "targets-held-template",
    "targets-failed-expression",
    "targets-failed-template",
    "actions-entry-held",
    "actions-entry-failed",
    "actions-entry-absent",
    "actions-held-expression",
    "actions-held-template",
    "actions-failed-expression",
    "actions-failed-template",
    "reads-vars",
];
