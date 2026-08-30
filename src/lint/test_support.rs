//! Shared helpers for the linter's unit tests.
//!
//! Every rule test starts from manifest text, so the helpers here run the real
//! pipeline — parse, expand, render, lower — and then lint the result. A rule
//! test therefore exercises the same artefacts the command does.
//!
//! The functions are fallible and the assertions are macros. A fixture that
//! cannot be built is a broken test rather than a verdict, so it propagates;
//! the macros expand in the calling test, which keeps a failure's line number
//! pointing at the case that failed rather than at this file.

use anyhow::{Context, Result};

use crate::ir::BuildGraph;
use crate::lint::finding::Finding;
use crate::lint::policy::Policy;
use crate::lint::severity::Severity;
use crate::lint::{Outcome, Request, analyse};
use crate::manifest;

/// Lint `yaml` under the registry defaults.
///
/// # Errors
///
/// Returns an error when the fixture does not parse, lower, or index.
pub fn lint(yaml: &str) -> Result<Outcome> {
    lint_with(yaml, &Policy::defaults())
}

/// Lint `yaml` under `policy`.
///
/// # Errors
///
/// Returns an error when the fixture does not parse, lower, or index.
pub fn lint_with(yaml: &str, policy: &Policy) -> Result<Outcome> {
    let parsed = manifest::from_str(yaml).context("fixture manifest should parse")?;
    let graph = BuildGraph::from_manifest(&parsed).context("fixture manifest should lower")?;
    analyse(
        Request {
            source: yaml.to_owned(),
            manifest: &parsed,
            graph: &graph,
        },
        policy,
    )
    .map_err(|failure| anyhow::anyhow!("fixture manifest should index: {}", failure.message))
}

/// Every category selector, set to `off`.
///
/// Isolating one rule means disabling every category and then re-enabling that
/// rule, so a finding cannot arrive from a neighbour.
fn silence_every_category() -> Vec<String> {
    crate::lint::Category::ALL
        .into_iter()
        .map(|category| format!("{}=off", category.as_str()))
        .collect()
}

/// Lint `yaml` with only `rule` enabled.
///
/// # Errors
///
/// Returns an error when `rule` is not registered or the fixture does not
/// compile.
pub fn lint_only(yaml: &str, rule: &str) -> Result<Vec<Finding>> {
    let mut selectors = silence_every_category();
    selectors.push(format!("{rule}=warning"));
    let policy = Policy::resolve(&selectors)
        .map_err(|error| anyhow::anyhow!("selectors should resolve: {}", error.message()))?;
    Ok(lint_with(yaml, &policy)?.findings)
}

/// Report the messages `rule` produced for `yaml`.
///
/// # Errors
///
/// Returns an error when `rule` is not registered or the fixture does not
/// compile.
pub fn messages_for(yaml: &str, rule: &str) -> Result<Vec<String>> {
    Ok(lint_only(yaml, rule)?
        .iter()
        .map(Finding::display_message)
        .collect())
}

/// Report the source text each finding of `rule` points at.
///
/// # Errors
///
/// Returns an error when `rule` is not registered or the fixture does not
/// compile.
pub fn spans_for(yaml: &str, rule: &str) -> Result<Vec<String>> {
    Ok(lint_only(yaml, rule)?
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
        .collect())
}

/// Report the severities the findings of `rule` carry.
///
/// # Errors
///
/// Returns an error when `rule` is not registered or the fixture does not
/// compile.
pub fn severities_for(yaml: &str, rule: &str) -> Result<Vec<Severity>> {
    Ok(lint_only(yaml, rule)?
        .iter()
        .map(|finding| finding.severity)
        .collect())
}

/// Report the rules that fired for `yaml` under the registry defaults.
///
/// A rule that reasons about other rules' findings — `unused-suppression` —
/// must be measured with the default policy rather than in isolation.
///
/// # Errors
///
/// Returns an error when the fixture does not compile.
pub fn rules_fired(yaml: &str) -> Result<Vec<&'static str>> {
    Ok(lint(yaml)?
        .findings
        .iter()
        .map(|finding| finding.meta.name)
        .collect())
}

/// Lint a fixture under the registry defaults, failing the calling test when
/// the fixture itself is broken.
#[macro_export]
macro_rules! lint_fixture {
    ($yaml:expr $(,)?) => {
        $crate::lint::test_support::lint($yaml).expect("fixture should compile")
    };
}

/// Lint a fixture under an explicit policy.
#[macro_export]
macro_rules! lint_fixture_with {
    ($yaml:expr, $policy:expr $(,)?) => {
        $crate::lint::test_support::lint_with($yaml, $policy).expect("fixture should compile")
    };
}

/// Report the messages one rule produced for a fixture.
#[macro_export]
macro_rules! lint_messages {
    ($yaml:expr, $rule:expr $(,)?) => {
        $crate::lint::test_support::messages_for($yaml, $rule).expect("fixture should compile")
    };
}

/// Report the source text each of one rule's findings points at.
#[macro_export]
macro_rules! lint_spans {
    ($yaml:expr, $rule:expr $(,)?) => {
        $crate::lint::test_support::spans_for($yaml, $rule).expect("fixture should compile")
    };
}

/// Report how many findings one rule produced for a fixture.
#[macro_export]
macro_rules! lint_count {
    ($yaml:expr, $rule:expr $(,)?) => {
        $crate::lint::test_support::lint_only($yaml, $rule)
            .expect("fixture should compile")
            .len()
    };
}

/// Report the rules that fired for a fixture under the registry defaults.
#[macro_export]
macro_rules! lint_rules_fired {
    ($yaml:expr $(,)?) => {
        $crate::lint::test_support::rules_fired($yaml).expect("fixture should compile")
    };
}

/// Assert that one rule fires exactly `$count` times for a fixture.
#[macro_export]
macro_rules! assert_lint_fires {
    ($yaml:expr, $rule:expr, $count:expr $(,)?) => {{
        let messages = $crate::lint_messages!($yaml, $rule);
        assert_eq!(
            messages.len(),
            $count,
            "`{}` should fire {} time(s); reported {messages:?}",
            $rule,
            $count
        );
    }};
}

/// Assert that one rule reports nothing for a fixture.
#[macro_export]
macro_rules! assert_lint_silent {
    ($yaml:expr, $rule:expr $(,)?) => {{
        let messages = $crate::lint_messages!($yaml, $rule);
        assert!(
            messages.is_empty(),
            "`{}` should report nothing; reported {messages:?}",
            $rule
        );
    }};
}

/// Assert that one rule fires under the registry defaults.
#[macro_export]
macro_rules! assert_lint_fires_by_default {
    ($yaml:expr, $rule:expr $(,)?) => {{
        let fired = $crate::lint_rules_fired!($yaml);
        assert!(
            fired.contains(&$rule),
            "`{}` should fire under the defaults; fired {fired:?}",
            $rule
        );
    }};
}

/// Assert that one rule stays silent under the registry defaults.
#[macro_export]
macro_rules! assert_lint_silent_by_default {
    ($yaml:expr, $rule:expr $(,)?) => {{
        let fired = $crate::lint_rules_fired!($yaml);
        assert!(
            !fired.contains(&$rule),
            "`{}` should stay silent under the defaults; fired {fired:?}",
            $rule
        );
    }};
}
