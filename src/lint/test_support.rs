//! Shared helpers for the linter's unit tests.
//!
//! Every rule test starts from manifest text, so the helpers here run the real
//! pipeline — parse, expand, render, lower — and then lint the result. A rule
//! test therefore exercises the same artefacts the command does.

use crate::ir::BuildGraph;
use crate::lint::finding::Finding;
use crate::lint::policy::Policy;
use crate::lint::severity::Severity;
use crate::lint::{Outcome, Request, analyse};
use crate::manifest;

/// Lint `yaml` under the registry defaults.
///
/// # Panics
///
/// Panics when the manifest does not parse or lower, which means the test
/// fixture itself is wrong.
#[must_use]
pub fn lint(yaml: &str) -> Outcome {
    lint_with(yaml, &Policy::defaults())
}

/// Lint `yaml` under `policy`.
///
/// # Panics
///
/// Panics when the manifest does not parse or lower.
#[must_use]
pub fn lint_with(yaml: &str, policy: &Policy) -> Outcome {
    let parsed = manifest::from_str(yaml).expect("fixture manifest should parse");
    let graph = BuildGraph::from_manifest(&parsed).expect("fixture manifest should lower");
    analyse(
        Request {
            source: yaml.to_owned(),
            manifest: &parsed,
            graph: &graph,
        },
        policy,
    )
    .expect("fixture manifest should index")
}

/// Lint `yaml` with only `rule` enabled.
///
/// # Panics
///
/// Panics when `rule` is not registered, or when the fixture does not compile.
#[must_use]
pub fn lint_only(yaml: &str, rule: &str) -> Vec<Finding> {
    let selectors = [
        "correctness=off",
        "caching=off",
        "portability=off",
        "determinism=off",
        "redundancy=off",
        "hygiene=off",
        "clarity=off",
        "migration=off",
        "suppression=off",
    ];
    let mut policy_selectors: Vec<String> =
        selectors.iter().map(|entry| (*entry).to_owned()).collect();
    policy_selectors.push(format!("{rule}=warning"));
    let policy = Policy::resolve(&policy_selectors).expect("selectors should resolve");
    lint_with(yaml, &policy).findings
}

/// Report the messages `rule` produced for `yaml`.
///
/// # Panics
///
/// Panics when `rule` is not registered, or when the fixture does not compile.
#[must_use]
pub fn messages_for(yaml: &str, rule: &str) -> Vec<String> {
    lint_only(yaml, rule)
        .iter()
        .map(Finding::display_message)
        .collect()
}

/// Report the source text each finding of `rule` points at.
///
/// # Panics
///
/// Panics when `rule` is not registered, or when the fixture does not compile.
#[must_use]
pub fn spans_for(yaml: &str, rule: &str) -> Vec<String> {
    lint_only(yaml, rule)
        .iter()
        .map(|finding| {
            finding.span().map_or_else(
                || "<no span>".to_owned(),
                |span| {
                    yaml.get(span.start..span.end)
                        .unwrap_or_default()
                        .to_owned()
                },
            )
        })
        .collect()
}

/// Report the rules that fired for `yaml` under the registry defaults.
///
/// A rule that reasons about other rules' findings — `unused-suppression` —
/// must be measured with the default policy rather than in isolation.
///
/// # Panics
///
/// Panics when the fixture does not compile.
#[must_use]
pub fn rules_fired(yaml: &str) -> Vec<&'static str> {
    lint(yaml)
        .findings
        .iter()
        .map(|finding| finding.meta.name)
        .collect()
}

/// Assert that `rule` fires under the registry defaults.
///
/// # Panics
///
/// Panics when it does not, naming everything that did.
pub fn assert_fires_by_default(yaml: &str, rule: &str) {
    let fired = rules_fired(yaml);
    assert!(
        fired.contains(&rule),
        "`{rule}` should fire under the defaults; fired {fired:?}"
    );
}

/// Assert that `rule` stays silent under the registry defaults.
///
/// # Panics
///
/// Panics when it fires, naming everything that did.
pub fn assert_silent_by_default(yaml: &str, rule: &str) {
    let fired = rules_fired(yaml);
    assert!(
        !fired.contains(&rule),
        "`{rule}` should stay silent under the defaults; fired {fired:?}"
    );
}

/// Report how many findings `rule` produced for `yaml`.
///
/// # Panics
///
/// Panics when `rule` is not registered, or when the fixture does not compile.
#[must_use]
pub fn count_for(yaml: &str, rule: &str) -> usize {
    lint_only(yaml, rule).len()
}

/// Assert that `rule` fires exactly `expected` times for `yaml`.
///
/// # Panics
///
/// Panics when the count differs, naming the messages that were reported so a
/// failure explains itself without a second run.
pub fn assert_fires(yaml: &str, rule: &str, expected: usize) {
    let messages = messages_for(yaml, rule);
    assert_eq!(
        messages.len(),
        expected,
        "`{rule}` should fire {expected} time(s); reported {messages:?}"
    );
}

/// Assert that `rule` reports nothing for `yaml`.
///
/// # Panics
///
/// Panics when the rule fires, naming what it reported.
pub fn assert_silent(yaml: &str, rule: &str) {
    let messages = messages_for(yaml, rule);
    assert!(
        messages.is_empty(),
        "`{rule}` should report nothing; reported {messages:?}"
    );
}

/// Report the severities the findings of `rule` carry.
///
/// # Panics
///
/// Panics when `rule` is not registered, or when the fixture does not compile.
#[must_use]
pub fn severities_for(yaml: &str, rule: &str) -> Vec<Severity> {
    lint_only(yaml, rule)
        .iter()
        .map(|finding| finding.severity)
        .collect()
}
