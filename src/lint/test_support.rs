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
