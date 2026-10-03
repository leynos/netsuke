//! Tests for policy resolution from `--rule` selectors.

use rstest::rstest;

use super::{Policy, PolicyError};
use crate::lint::registry;
use crate::lint::rule::Category;
use crate::lint::severity::{DefaultSeverity, Severity};

/// Resolve `selectors`, failing the calling test when one is rejected.
macro_rules! resolve {
    ($selectors:expr) => {
        Policy::resolve($selectors).expect("selectors should resolve")
    };
}

#[test]
fn defaults_match_the_registry() {
    let policy = Policy::defaults();
    for meta in registry::all_meta() {
        assert_eq!(
            policy.severity_of(meta.name),
            meta.default_severity.severity(),
            "`{}` should start at its registered default",
            meta.name
        );
    }
}

#[test]
fn a_rule_selector_overrides_one_rule() {
    let policy = resolve!(&["background-job=error"]);
    assert_eq!(policy.severity_of("background-job"), Some(Severity::Error));
    assert_eq!(
        policy.severity_of("bashism"),
        Some(Severity::Warning),
        "an unrelated rule should keep its default"
    );
}

#[test]
fn a_category_selector_overrides_every_rule_in_it() {
    let policy = resolve!(&["hygiene=off"]);
    for meta in registry::all_meta().filter(|meta| meta.category == Category::Hygiene) {
        assert_eq!(
            policy.severity_of(meta.name),
            None,
            "`{}` should be disabled by its category",
            meta.name
        );
    }
}

/// Selectors apply in order, so a rule selector narrows a category selector
/// and not the other way round.
#[test]
fn later_selectors_win() {
    let narrowed = resolve!(&["hygiene=off", "unused-var=error"]);
    assert_eq!(narrowed.severity_of("unused-var"), Some(Severity::Error));
    assert_eq!(narrowed.severity_of("unused-rule"), None);

    let widened = resolve!(&["unused-var=error", "hygiene=off"]);
    assert_eq!(widened.severity_of("unused-var"), None);
}

#[test]
fn a_selector_enables_an_opt_in_rule() {
    let opt_in = registry::all_meta()
        .find(|meta| meta.default_severity == DefaultSeverity::Off)
        .expect("the registry should ship an opt-in rule");
    assert_eq!(Policy::defaults().severity_of(opt_in.name), None);
    assert_eq!(
        resolve!(&[&format!("{}=warning", opt_in.name)]).severity_of(opt_in.name),
        Some(Severity::Warning)
    );
}

#[test]
fn disabling_every_category_disables_every_rule() {
    let selectors: Vec<String> = Category::ALL
        .into_iter()
        .map(|category| format!("{}=off", category.as_str()))
        .collect();
    assert!(resolve!(&selectors.iter().map(String::as_str).collect::<Vec<_>>()).is_empty());
}

/// A typo in continuous-integration configuration must fail loudly rather than
/// silently widening or narrowing the run.
#[rstest]
#[case("background-job", PolicyError::Malformed { selector: "background-job".to_owned() })]
#[case("no-such-rule=error", PolicyError::UnknownName { name: "no-such-rule".to_owned() })]
#[case(
    "background-job=fatal",
    PolicyError::UnknownSeverity {
        name: "background-job".to_owned(),
        severity: "fatal".to_owned(),
    }
)]
fn an_invalid_selector_is_rejected(#[case] selector: &str, #[case] expected: PolicyError) {
    let error = Policy::resolve(&[selector]).expect_err("the selector should be rejected");
    assert_eq!(error, expected);
    assert!(
        !error.message().is_empty(),
        "the rejection should explain itself"
    );
}

#[test]
fn whitespace_around_a_selector_is_ignored() {
    assert_eq!(
        resolve!(&[" background-job = error "]).severity_of("background-job"),
        Some(Severity::Error)
    );
}

#[test]
fn the_severity_rejection_lists_the_accepted_values() {
    let error = Policy::resolve(&["background-job=fatal"]).expect_err("should be rejected");
    let message = error.message();
    for value in ["off", "advice", "warning", "error"] {
        assert!(
            message.contains(value),
            "the rejection should list `{value}`, got {message}"
        );
    }
}
