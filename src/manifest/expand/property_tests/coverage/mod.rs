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
    static_condition_entry_spec_strategy,
};
use classes::{DOCUMENT_CLASSES, ENTRY_CLASSES, INJECT_CLASSES, STATIC_CLASSES};
use classifiers::{
    document_classes, entry_classes, forced_control_classes, injection_classes, static_classes,
};
use proptest::strategy::Strategy;

mod classes;
mod classifiers;
use proptest::test_runner::{Config, RngSeed, TestRunner};
use std::cell::RefCell;
use std::collections::BTreeMap;

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

/// One classification tally.
type Tally = BTreeMap<&'static str, usize>;

/// Add one class to a tally.
fn count(tally: &mut Tally, class: &'static str) {
    *tally.entry(class).or_insert(0) += 1;
}

/// Count the classes of a fixed-seed sample of one strategy.
fn tally<S: Strategy>(
    strategy: &S,
    classify: impl Fn(&S::Value, &mut Tally),
) -> Result<Tally, String> {
    let mut runner = TestRunner::new(Config {
        cases: COVERAGE_CASES,
        rng_seed: RngSeed::Fixed(COVERAGE_SEED),
        ..Config::default()
    });
    let counts = RefCell::new(Tally::new());
    runner
        .run(strategy, |value| {
            classify(&value, &mut counts.borrow_mut());
            Ok(())
        })
        .map_err(|error| error.to_string())?;
    Ok(counts.into_inner())
}

/// Fail when any required class reaches less than the floor.
fn check_floors(tally: &Tally, required: &[&'static str], total: usize) -> Result<(), String> {
    let floor = total.saturating_mul(MINIMUM_CLASS_PERCENT).div_euclid(100);
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
    let tally = tally(&entry_spec_strategy(), entry_classes)?;
    report("entry", &tally);
    check_floors(&tally, ENTRY_CLASSES, COVERAGE_CASES as usize)
}

#[test]
fn foreach_generator_reaches_every_injection_class() -> Result<(), String> {
    let tally = tally(&foreach_entry_spec_strategy(), injection_classes)?;
    report("inject", &tally);
    check_floors(&tally, INJECT_CLASSES, COVERAGE_CASES as usize)
}

#[test]
fn static_condition_generator_reaches_every_verdict_class() -> Result<(), String> {
    let tally = tally(&static_condition_entry_spec_strategy(), static_classes)?;
    report("static", &tally);
    check_floors(&tally, STATIC_CLASSES, COVERAGE_CASES as usize)
}

#[test]
fn document_generator_reaches_every_section_class() -> Result<(), String> {
    let tally = tally(&document_spec_strategy(), document_classes)?;
    report("document", &tally);
    check_floors(&tally, DOCUMENT_CLASSES, COVERAGE_CASES as usize)
}

/// Verify the floor check reports a class the sample never reaches.
///
/// The sample is forced to place the control block last, so the class "two or
/// more keys follow the control block" cannot occur. The check must then fail
/// and name that class.
#[test]
fn the_threshold_check_reports_an_unreached_class() -> Result<(), String> {
    let observed = tally(&foreach_entry_spec_strategy(), forced_control_classes)?;
    let unreached = "control-two-or-more-following";
    let failure = check_floors(
        &observed,
        &["control-last", "control-one-following", unreached],
        COVERAGE_CASES as usize,
    )
    .expect_err("a sample with the forced layout must fail the floor check");
    if !failure.contains(unreached) {
        return Err(format!(
            "the floor check failed but did not name `{unreached}`: {failure}"
        ));
    }
    Ok(())
}
