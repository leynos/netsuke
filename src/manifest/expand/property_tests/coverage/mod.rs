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

use super::model::expected_filtered;
use super::strategy::{
    Items, Section, document_spec_strategy, entry_spec_strategy, foreach_entry_spec_strategy,
    static_condition_entry_spec_strategy,
};
use crate::manifest::ManifestValue;
use classes::{DOCUMENT_CLASSES, ENTRY_CLASSES, INJECT_CLASSES, STATIC_CLASSES};
use proptest::strategy::Strategy;

mod classes;
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

/// Count the classes of a fixed-seed sample of one strategy.
fn tally<S: Strategy>(
    strategy: S,
    classify: impl Fn(&S::Value, &mut Tally),
) -> Result<Tally, String> {
    let mut runner = TestRunner::new(Config {
        cases: COVERAGE_CASES,
        rng_seed: RngSeed::Fixed(COVERAGE_SEED),
        ..Config::default()
    });
    let counts = RefCell::new(Tally::new());
    runner
        .run(&strategy, |value| {
            classify(&value, &mut counts.borrow_mut());
            Ok(())
        })
        .map_err(|error| error.to_string())?;
    Ok(counts.into_inner())
}

/// Fail when any required class reaches less than the floor.
fn check_floors(tally: &Tally, required: &[&'static str], total: usize) -> Result<(), String> {
    let floor = total * MINIMUM_CLASS_PERCENT / 100;
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

/// Add one class to a tally.
fn count(tally: &mut Tally, class: &'static str) {
    *tally.entry(class).or_insert(0) += 1;
}

/// Count the JSON kind of one value, and whether it nests another container.
fn value_classes(value: &ManifestValue, depth: usize, tally: &mut Tally) {
    match value {
        ManifestValue::Null => count(tally, "value-null"),
        ManifestValue::Bool(_) => count(tally, "value-bool"),
        ManifestValue::Number(_) => count(tally, "value-number"),
        ManifestValue::String(text) => {
            count(tally, "value-string");
            if text.contains("{{") {
                count(tally, "value-jinja-string");
            }
        }
        ManifestValue::Array(items) => {
            count(tally, "value-array");
            if depth >= 1 {
                count(tally, "value-nested");
            }
            for item in items {
                value_classes(item, depth + 1, tally);
            }
        }
        ManifestValue::Object(map) => {
            count(tally, "value-object");
            if depth >= 1 {
                count(tally, "value-nested");
            }
            for value in map.values() {
                value_classes(value, depth + 1, tally);
            }
        }
    }
}

#[test]
fn entry_generator_reaches_every_material_class() -> Result<(), String> {
    let tally = tally(entry_spec_strategy(), |spec, tally| {
        let body = &spec.body;
        count(tally, spec.section.name());
        count(
            tally,
            if body.is_foreach() {
                "foreach"
            } else {
                "static"
            },
        );
        count(tally, if body.has_vars() { "vars" } else { "no-vars" });
        match body.condition() {
            Some(condition) => {
                count(tally, "condition");
                count(
                    tally,
                    if body.condition_is_template() {
                        "condition-template"
                    } else {
                        "condition-expression"
                    },
                );
                if condition.variable().is_some() {
                    count(tally, "condition-reads-vars");
                }
                if condition.reads_index() {
                    count(tally, "condition-reads-index");
                }
                if condition.reads_item() {
                    count(tally, "condition-reads-item");
                }
            }
            None => count(tally, "no-condition"),
        }
        match body.following_key_count() {
            0 => count(tally, "control-last"),
            1 => count(tally, "control-one-following"),
            _ => count(tally, "control-two-or-more-following"),
        }
        if let Some(items) = body.items() {
            count(
                tally,
                if items.len() == 0 {
                    "foreach-empty"
                } else {
                    "foreach-non-empty"
                },
            );
            count(
                tally,
                match body.foreach_form() {
                    Some(super::strategy::ForeachForm::InlineArray) => "foreach-array",
                    Some(super::strategy::ForeachForm::ListLiteral) => "foreach-string",
                    None => "foreach-missing-form",
                },
            );
        }
        for (_, value) in body.fields() {
            value_classes(value, 0, tally);
        }
        for key in body.vars_keys() {
            if let Some(value) = body.vars_value(key) {
                value_classes(value, 0, tally);
            }
        }
    })?;
    println!("entry classes: {tally:?}");
    check_floors(&tally, ENTRY_CLASSES, COVERAGE_CASES as usize)
}

#[test]
fn foreach_generator_reaches_every_injection_class() -> Result<(), String> {
    let tally = tally(foreach_entry_spec_strategy(), |spec, tally| {
        let body = &spec.body;
        if let Some(form) = body.foreach_form() {
            count(
                tally,
                match form {
                    super::strategy::ForeachForm::InlineArray => "foreach-array",
                    super::strategy::ForeachForm::ListLiteral => "foreach-string",
                },
            );
        }
        let keys = body.vars_keys();
        if keys.contains(&"item") {
            count(tally, "user-item-collision");
        }
        if keys.contains(&"index") {
            count(tally, "user-index-collision");
        }
        let Some(items) = body.items() else {
            return;
        };
        let included: Vec<usize> = (0..items.len())
            .filter(|index| body.condition_holds(items.value_at(*index).as_i64(), *index))
            .collect();
        if items.len() == 0 {
            count(tally, "inject-empty");
        } else if included.len() == items.len() {
            count(tally, "inject-all-included");
        } else if included.is_empty() {
            count(tally, "inject-all-excluded");
        }
        if (0..items.len())
            .any(|index| !included.contains(&index) && included.iter().any(|kept| *kept > index))
        {
            count(tally, "inject-gap");
        }
        match items {
            Items::Integers(_) => count(tally, "item-number"),
            Items::Arbitrary(values) => {
                for value in values {
                    match value {
                        ManifestValue::Null => {
                            count(tally, "item-null");
                            if body.foreach_form()
                                == Some(super::strategy::ForeachForm::ListLiteral)
                            {
                                count(tally, "item-null-string");
                            }
                        }
                        ManifestValue::Bool(_) => count(tally, "item-bool"),
                        ManifestValue::Number(_) => count(tally, "item-number"),
                        ManifestValue::String(_) => count(tally, "item-string"),
                        ManifestValue::Array(_) => count(tally, "item-array"),
                        ManifestValue::Object(_) => count(tally, "item-object"),
                    }
                }
            }
        }
    })?;
    println!("inject classes: {tally:?}");
    check_floors(&tally, INJECT_CLASSES, COVERAGE_CASES as usize)
}

#[test]
fn static_condition_generator_reaches_every_verdict_class() -> Result<(), String> {
    let tally = tally(static_condition_entry_spec_strategy(), |spec, tally| {
        let body = &spec.body;
        let verdict_holds = expected_filtered(body).is_empty();
        let template = body.condition_is_template();
        let class = match (spec.section, verdict_holds, template) {
            (Section::Targets, true, false) => "targets-held-expression",
            (Section::Targets, true, true) => "targets-held-template",
            (Section::Targets, false, false) => "targets-failed-expression",
            (Section::Targets, false, true) => "targets-failed-template",
            (Section::Actions, true, false) => "actions-held-expression",
            (Section::Actions, true, true) => "actions-held-template",
            (Section::Actions, false, false) => "actions-failed-expression",
            (Section::Actions, false, true) => "actions-failed-template",
        };
        count(tally, class);
        if body
            .condition()
            .is_some_and(|condition| condition.variable().is_some())
        {
            count(tally, "reads-vars");
        }
    })?;
    println!("static classes: {tally:?}");
    check_floors(&tally, STATIC_CLASSES, COVERAGE_CASES as usize)
}

#[test]
fn document_generator_reaches_every_section_class() -> Result<(), String> {
    let tally = tally(document_spec_strategy(), |spec, tally| {
        if !spec.targets.is_empty() {
            count(tally, "targets-entries");
        }
        if !spec.actions.is_empty() {
            count(tally, "actions-entries");
        }
        for entry in spec.all_entries() {
            count(
                tally,
                if entry.body.is_foreach() {
                    "document-foreach"
                } else {
                    "document-static"
                },
            );
            if entry.body.has_condition() {
                count(tally, "document-condition");
            }
        }
    })?;
    println!("document classes: {tally:?}");
    check_floors(&tally, DOCUMENT_CLASSES, COVERAGE_CASES as usize)
}

/// Verify the floor check reports a class the sample never reaches.
///
/// The sample is forced to place the control block last, so the class "two or
/// more keys follow the control block" cannot occur. The check must then fail
/// and name that class.
#[test]
fn the_threshold_check_reports_an_unreached_class() -> Result<(), String> {
    let observed = tally(foreach_entry_spec_strategy(), |spec, tally| {
        let forced = spec.body.clone().with_control_last();
        match forced.following_key_count() {
            0 => count(tally, "control-last"),
            1 => count(tally, "control-one-following"),
            _ => count(tally, "control-two-or-more-following"),
        }
    })?;
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
