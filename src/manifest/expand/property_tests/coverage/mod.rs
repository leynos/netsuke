//! `OBL-GEN`: each generator still reaches every class its properties need.
//!
//! A property is only as strong as the inputs it is handed. If a generator
//! quietly stopped emitting, say, static entries, the property would keep
//! passing over a sample that never tested them. Each test here classifies a
//! fixed-seed sample of one strategy and fails when any required class falls
//! below its floor.
//!
//! The seed is fixed on purpose: a coverage claim is about the generator, not
//! about one draw, so the verdict and the counts must be identical on every
//! run. The threshold check has a negative control, so a check that could
//! never fail is visible as such.

use super::strategy::{
    document_spec_strategy, entry_spec_strategy, foreach_entry_spec_strategy,
    static_condition_entry_spec_strategy, static_document_spec_strategy,
};
use classes::{
    DOCUMENT_CLASSES, ENTRY_CLASSES, FOREACH_ORDER_CLASSES, INJECT_CLASSES, STATIC_CLASSES,
    STATIC_DOCUMENT_CLASSES,
};
use classifiers::{
    document_classes, entry_classes, forced_control_classes, injection_classes, static_classes,
};
use proptest::strategy::Strategy;

mod classes;
mod classifiers;
use proptest::test_runner::{Config, RngSeed, TestRunner};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

/// Cases each classifier draws.
const COVERAGE_CASES: u32 = 512;

/// Seed for the classifiers' runner.
///
/// Arbitrary and free to change: what matters is that it is fixed, so the
/// counts are comparable between runs.
const COVERAGE_SEED: u64 = 0x432;

/// Share of draws each class must reach, in percent.
///
/// At five percent, a 128-case random-seed run of a property meets the class
/// with probability above 99.8%.
const MINIMUM_CLASS_PERCENT: usize = 5;

/// One classification tally, counting draws rather than occurrences.
type Tally = BTreeMap<&'static str, usize>;

/// The classes one draw belongs to.
///
/// A set, not a counter: a class floor is a share of *draws*, so a draw that
/// contains three fields of one kind contributes one hit, not three. Counting
/// occurrences would let a handful of draws satisfy a floor on their own.
type ClassSet = BTreeSet<&'static str>;

/// Record that one draw belongs to `class`.
///
/// Classifiers import this as `count`, because their call sites read as
/// "count this class for this draw".
fn count(classes: &mut ClassSet, class: &'static str) {
    classes.insert(class);
}

/// Count the classes of a fixed-seed sample of one strategy.
///
/// The classifier reports whether the draw is in the scope its classes
/// describe. A draw outside it, such as a static entry without a condition
/// when the classes are verdicts, contributes to neither the classes nor the
/// denominator: including it would dilute a share of draws into a share of a
/// larger set the class cannot occur in.
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
///
/// The repository denies `print_stdout`, and the tally is diagnostic evidence
/// rather than test output, so it goes through the `Write` API and a failure
/// to write it cannot fail the test.
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
///
/// The property draws whole documents of static entries, so the per-section
/// verdict classes the obligation names are exactly the document classifier's.
#[test]
fn static_document_generator_reaches_every_verdict_class() -> Result<(), String> {
    let (tally, draws) = tally(&static_document_spec_strategy(), document_classes)?;
    report("static-document", &tally);
    check_floors(&tally, STATIC_DOCUMENT_CLASSES, draws)
}

/// Classify the transformed entry the foreach order property actually expands.
///
/// That property removes `when` before expansion, which changes how many keys
/// follow the remaining control key; classifying the untransformed entry would
/// credit a class the property never reaches.
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
///
/// The sample is forced to place the control block last, so the class "two or
/// more keys follow the control block" cannot occur. The check must then fail
/// and name that class.
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
