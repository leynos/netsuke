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
            for nested in map.values() {
                value_classes(nested, depth + 1, tally);
            }
        }
    }
}

/// Classify one general entry's section, control-key layout, and bodies.
fn entry_classes(spec: &super::strategy::EntrySpec, tally: &mut Tally) {
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
    condition_classes(body, tally);
    count(tally, control_class(body));
    foreach_classes(body, tally);
    value_and_vars_classes(body, tally);
}

/// Classify a condition's presence, form, and free names.
fn condition_classes(body: &super::strategy::BodySpec, tally: &mut Tally) {
    let Some(condition) = body.condition() else {
        count(tally, "no-condition");
        return;
    };
    count(tally, "condition");
    count(tally, condition_form_class(body));
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

/// Name the condition's spelling class.
fn condition_form_class(body: &super::strategy::BodySpec) -> &'static str {
    if body.condition_is_template() {
        "condition-template"
    } else {
        "condition-expression"
    }
}

/// Name the class of keys that follow the control block.
fn control_class(body: &super::strategy::BodySpec) -> &'static str {
    match body.following_key_count() {
        0 => "control-last",
        1 => "control-one-following",
        _ => "control-two-or-more-following",
    }
}

/// Classify the foreach spelling and whether the value list is empty.
fn foreach_classes(body: &super::strategy::BodySpec, tally: &mut Tally) {
    let Some(items) = body.items() else {
        return;
    };
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

/// Classify every authored field and `vars` value by JSON kind.
fn value_and_vars_classes(body: &super::strategy::BodySpec, tally: &mut Tally) {
    for (_, value) in body.fields() {
        value_classes(value, 0, tally);
    }
    for key in body.vars_keys() {
        if let Some(value) = body.vars_value(key) {
            value_classes(value, 0, tally);
        }
    }
}

#[test]
fn entry_generator_reaches_every_material_class() -> Result<(), String> {
    let tally = tally(&entry_spec_strategy(), entry_classes)?;
    report("entry", &tally);
    check_floors(&tally, ENTRY_CLASSES, COVERAGE_CASES as usize)
}

/// Count the item kinds one foreach entry's values reach.
fn item_classes(body: &super::strategy::BodySpec, items: &Items, tally: &mut Tally) {
    let Items::Arbitrary(values) = items else {
        count(tally, "item-number");
        return;
    };
    let string_form = body.foreach_form() == Some(super::strategy::ForeachForm::ListLiteral);
    for value in values {
        let kind = match value {
            ManifestValue::Null => {
                if string_form {
                    count(tally, "item-null-string");
                }
                "item-null"
            }
            ManifestValue::Bool(_) => "item-bool",
            ManifestValue::Number(_) => "item-number",
            ManifestValue::String(_) => "item-string",
            ManifestValue::Array(_) => "item-array",
            ManifestValue::Object(_) => "item-object",
        };
        count(tally, kind);
    }
}

/// Classify one foreach entry for the injection obligations.
fn injection_classes(spec: &super::strategy::EntrySpec, tally: &mut Tally) {
    let body = &spec.body;
    if let Some(form) = body.foreach_form() {
        count(tally, foreach_form_class(form));
    }
    user_collisions(body, tally);
    let Some(items) = body.items() else {
        return;
    };
    let included: Vec<usize> = (0..items.len())
        .filter(|index| body.condition_holds(items.value_at(*index).as_i64(), *index))
        .collect();
    count(tally, inclusion_class(items.len(), &included));
    if has_gap(items.len(), &included) {
        count(tally, "inject-gap");
    }
    item_classes(body, items, tally);
}

/// Name the class of one `foreach` spelling.
fn foreach_form_class(form: super::strategy::ForeachForm) -> &'static str {
    match form {
        super::strategy::ForeachForm::InlineArray => "foreach-array",
        super::strategy::ForeachForm::ListLiteral => "foreach-string",
    }
}

/// Count a user `vars` key that collides with an injected name.
fn user_collisions(body: &super::strategy::BodySpec, tally: &mut Tally) {
    let keys = body.vars_keys();
    if keys.contains(&"item") {
        count(tally, "user-item-collision");
    }
    if keys.contains(&"index") {
        count(tally, "user-index-collision");
    }
}

/// Name the selection class of one iteration sequence.
fn inclusion_class(item_count: usize, included: &[usize]) -> &'static str {
    if item_count == 0 {
        "inject-empty"
    } else if included.len() == item_count {
        "inject-all-included"
    } else if included.is_empty() {
        "inject-all-excluded"
    } else {
        "inject-partial"
    }
}

/// Report whether an excluded item precedes an included one.
///
/// This is the only selection shape on which renumbering an injected index is
/// visible: a prefix-shaped selection keeps true and renumbered indexes equal.
fn has_gap(item_count: usize, included: &[usize]) -> bool {
    (0..item_count)
        .any(|index| !included.contains(&index) && included.iter().any(|kept| *kept > index))
}

#[test]
fn foreach_generator_reaches_every_injection_class() -> Result<(), String> {
    let tally = tally(&foreach_entry_spec_strategy(), injection_classes)?;
    report("inject", &tally);
    check_floors(&tally, INJECT_CLASSES, COVERAGE_CASES as usize)
}

#[test]
fn static_condition_generator_reaches_every_verdict_class() -> Result<(), String> {
    let tally = tally(&static_condition_entry_spec_strategy(), |spec, tally| {
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
    report("static", &tally);
    check_floors(&tally, STATIC_CLASSES, COVERAGE_CASES as usize)
}

#[test]
fn document_generator_reaches_every_section_class() -> Result<(), String> {
    let tally = tally(&document_spec_strategy(), |spec, tally| {
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
    let observed = tally(&foreach_entry_spec_strategy(), |spec, tally| {
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
