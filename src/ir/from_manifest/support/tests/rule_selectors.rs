//! Regression coverage for rejecting multi-name rule selectors.
//!
//! Rule resolution used to clone the whole selector and insertion-sort it
//! before building the rejection, so reverse-ordered input cost a quadratic
//! number of comparisons and swaps for a message that could never show more
//! than a handful of names. These tests drive the production entry point and
//! measure the comparisons it makes, so a return to full-list sorting fails
//! here rather than in a stalled validating process.

use super::super::sort_utils::{reset_string_comparisons, string_comparisons};
use super::super::*;
use test_support::fluent::normalize_fluent_isolates;
use test_support::{EnLocalizer, en_localizer};

/// Names large enough that a quadratic comparison count is unmistakable.
///
/// The selector is rejected before any rule lookup, so no rule map is needed.
const OVERSIZED_SELECTOR_LEN: usize = 1024;

/// Names the diagnostic is documented to report before summarising the rest.
///
/// Stated here rather than imported from the implementation, so the test pins
/// the published contract rather than agreeing with whatever the cap becomes.
const MAX_REPORTED_RULE_SELECTORS: usize = 8;

/// Build `count` reverse-ordered, zero-padded rule names.
///
/// Zero-padding makes lexical order the reverse of input order: a selector
/// merely echoed or partially sorted would not match the expected prefix.
fn reverse_ordered_names(count: usize) -> Vec<String> {
    (0..count)
        .rev()
        .map(|index| format!("name_{index:04}"))
        .collect()
}

/// Return the lexically smallest `count` names, in ascending order.
fn ascending_names(count: usize) -> Vec<String> {
    (0..count).map(|index| format!("name_{index:04}")).collect()
}

/// Reject an oversized selector after at most a linear number of comparisons.
#[rstest::rstest]
fn oversized_rule_selector_is_rejected_with_a_bounded_sample(en_localizer: EnLocalizer) {
    let _localizer = en_localizer;
    let names = reverse_ordered_names(OVERSIZED_SELECTOR_LEN);
    reset_string_comparisons();

    let error = resolve_rule(
        &StringOrList::List(names.clone()),
        &IrHashMap::default(),
        "app",
    )
    .expect_err("a selector naming many rules must be rejected");

    let comparisons = string_comparisons();
    let IrGenError::MultipleRules {
        target_name,
        rules,
        message,
    } = error
    else {
        panic!("a multi-name selector must report MultipleRules, got {error:?}");
    };

    assert_eq!(
        target_name, "app",
        "the rejection must preserve the target display name"
    );
    // The guard against a vacuous counter: zero comparisons would mean the
    // comparison seam stopped being driven at all.
    assert!(
        comparisons > 0,
        "the comparison counter must observe the retained-name comparisons"
    );
    let bound = OVERSIZED_SELECTOR_LEN * (MAX_REPORTED_RULE_SELECTORS + 1);
    assert!(
        comparisons <= bound,
        "selecting the reported sample must cost at most {bound} comparisons, made {comparisons}"
    );
    assert_eq!(
        rules,
        ascending_names(MAX_REPORTED_RULE_SELECTORS),
        "the rejection must report the lexically smallest names in ascending order"
    );

    // Fluent isolates every placeable with direction marks, which would sit
    // between the numeral and its noun in the rendered text.
    let rendered = normalize_fluent_isolates(&message.to_string());
    let omitted = OVERSIZED_SELECTOR_LEN - MAX_REPORTED_RULE_SELECTORS;
    assert!(
        rendered.contains(&format!("{omitted} more names")),
        "the message must report the {omitted} omitted names: {rendered}"
    );
}

/// Keep the existing diagnostic byte-identical for a selector small enough to
/// report in full.
#[rstest::rstest]
fn small_rule_selector_keeps_the_untruncated_diagnostic(en_localizer: EnLocalizer) {
    let _localizer = en_localizer;
    let names = vec!["c".to_owned(), "a".to_owned(), "b".to_owned()];
    let error = resolve_rule(&StringOrList::List(names), &IrHashMap::default(), "app")
        .expect_err("a selector naming three rules must be rejected");
    let IrGenError::MultipleRules { rules, message, .. } = error else {
        panic!("a multi-name selector must report MultipleRules, got {error:?}");
    };

    assert_eq!(
        rules,
        vec!["a".to_owned(), "b".to_owned(), "c".to_owned()],
        "a reportable selector must still be sorted ascending"
    );
    let expected = localization::message(keys::IR_MULTIPLE_RULES)
        .with_arg("target", "app")
        .with_arg("rules", format!("{rules:?}"));
    assert_eq!(
        message.to_string(),
        expected.to_string(),
        "a selector with nothing omitted must render the existing message unchanged"
    );
}
