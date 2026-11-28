//! Generator self-checks: reachability and legality of what is drawn.

use super::generate::entry_spec_strategy;
use super::layout::ControlLayout;
use super::*;
use proptest::sample::Index;
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

/// Build a two-control entry with the given slots and order.
fn two_control_body(
    first_slot: ControlLayout,
    second_slot: ControlLayout,
    order: ControlOrder,
) -> Result<BodySpec, String> {
    Ok(BodySpec {
        fields: vec![("name".to_owned(), ManifestValue::String("x".to_owned()))],
        vars: None,
        vars_position: index_for_unused_vars()?,
        foreach: Some((ForeachForm::InlineArray, Items::Integers(vec![1]))),
        when: Some((Condition::Literal(true), false)),
        control_layout: first_slot,
        second_control_layout: second_slot,
        control_order: order,
        bindings: Bindings::with_count(0, 0),
    })
}

/// Render the control keys of one body, in order.
fn rendered_control_keys(body: &BodySpec) -> Vec<String> {
    body.control_key_sequence()
}

#[rstest::rstest]
#[case::foreach_first(ControlOrder::ForeachFirst, &["foreach", "when"])]
#[case::when_first(ControlOrder::WhenFirst, &["when", "foreach"])]
fn tied_control_slots_keep_the_recorded_order(
    #[case] order: ControlOrder,
    #[case] expected: &[&str],
) -> Result<(), String> {
    let body = two_control_body(ControlLayout::Last, ControlLayout::Last, order)?;
    let rendered = rendered_control_keys(&body);
    if rendered != expected {
        return Err(format!(
            "a tied layout rendered {rendered:?}, expected {expected:?}"
        ));
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    /// Both draw orders render the key written first before the other.
    #[test]
    fn independently_drawn_slots_render_in_the_recorded_order(
        first in any::<Index>(),
        second in any::<Index>(),
        order in ControlOrder::strategy(),
    ) {
        let body = two_control_body(
            ControlLayout::drawn(first),
            ControlLayout::drawn(second),
            order,
        )
        .map_err(TestCaseError::fail)?;
        let expected: &[&str] = match order {
            ControlOrder::ForeachFirst => &["foreach", "when"],
            ControlOrder::WhenFirst => &["when", "foreach"],
        };
        prop_assert_eq!(
            rendered_control_keys(&body),
            expected.iter().map(|key| (*key).to_owned()).collect::<Vec<_>>()
        );
    }
}

/// Draw one `Index` for a body whose `vars` is absent, where it is unused.
fn index_for_unused_vars() -> Result<Index, String> {
    let mut runner = TestRunner::deterministic();
    any::<Index>()
        .new_tree(&mut runner)
        .map(|tree| tree.current())
        .map_err(|error| error.to_string())
}
