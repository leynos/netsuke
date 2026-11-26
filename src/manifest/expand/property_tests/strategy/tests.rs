//! Generator self-checks: reachability and legality of what is drawn.

use super::generate::entry_spec_strategy;
use super::*;
use proptest::strategy::ValueTree;
use proptest::test_runner::TestRunner;
use std::collections::BTreeSet;

/// Draw one spec from a strategy, failing the test on a runner error.
fn draw<S: Strategy>(strategy: S, runner: &mut TestRunner) -> Result<S::Value, String> {
    strategy
        .new_tree(runner)
        .map(|tree| tree.current())
        .map_err(|error| error.to_string())
}

#[test]
fn generated_conditions_are_always_bound() -> Result<(), String> {
    let mut runner = TestRunner::deterministic();
    let strategy = entry_spec_strategy();
    for _ in 0..200 {
        let spec: EntrySpec = draw(&strategy, &mut runner)?;
        if let Some(condition) = spec.body.condition()
            && let Some(name) = condition.variable()
        {
            let keys = spec.body.vars_keys();
            if !keys.contains(&name) {
                return Err(format!(
                    "condition {condition:?} reads {name}, which vars {keys:?} do not bind"
                ));
            }
        }
        if !spec.body.is_foreach() && condition_reads_iteration(&spec) {
            return Err(format!(
                "static entry carries an iteration reference: {spec:?}"
            ));
        }
    }
    Ok(())
}

/// Report whether a spec's condition reads `item` or `index`.
fn condition_reads_iteration(spec: &EntrySpec) -> bool {
    spec.body
        .condition()
        .is_some_and(|condition| condition.reads_index() || condition.reads_item())
}

#[test]
fn every_section_and_foreach_form_is_reachable() -> Result<(), String> {
    let mut runner = TestRunner::deterministic();
    let strategy = entry_spec_strategy();
    let mut seen = BTreeSet::new();
    for _ in 0..600 {
        let spec: EntrySpec = draw(&strategy, &mut runner)?;
        seen.insert(spec.section.name());
        seen.insert(if spec.body.is_foreach() {
            "foreach"
        } else {
            "static"
        });
        if spec.body.has_condition() {
            seen.insert("condition");
        }
        if spec.body.has_vars() {
            seen.insert("vars");
        }
    }
    let expected = BTreeSet::from([
        "actions",
        "condition",
        "foreach",
        "static",
        "targets",
        "vars",
    ]);
    if seen != expected {
        return Err(format!("reached {seen:?}, expected {expected:?}"));
    }
    Ok(())
}
