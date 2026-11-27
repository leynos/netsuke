//! Plumbing shared by the expansion properties.

use crate::manifest::budget::ManifestBudget;
use crate::manifest::expand::{ExpansionReport, expand_foreach_with_budget};
use crate::manifest::{ManifestMap, ManifestValue};
use minijinja::{Environment, UndefinedBehavior};
use proptest::test_runner::TestCaseError;

/// Build the strict-undefined environment the production loader builds.
pub(super) fn strict_environment() -> Environment<'static> {
    let mut environment = Environment::new();
    environment.set_undefined_behavior(UndefinedBehavior::Strict);
    environment
}

/// Expand a document with a fresh default budget.
pub(super) fn expand_document(
    document: &mut ManifestValue,
) -> Result<ExpansionReport, TestCaseError> {
    let environment = strict_environment();
    let budget = ManifestBudget::default();
    expand_foreach_with_budget(document, &environment, &budget, false)
        .map_err(|error| TestCaseError::fail(format!("{error:#}")))
}

/// Read one section's entries, reporting absence as a test failure.
pub(super) fn section_entries<'a>(
    document: &'a ManifestValue,
    section: &str,
) -> Result<&'a [ManifestValue], TestCaseError> {
    document
        .get(section)
        .and_then(|value| value.as_array())
        .map(Vec::as_slice)
        .ok_or_else(|| TestCaseError::fail(format!("{section} sequence missing")))
}

/// Read one expanded entry as a map, reporting a non-object as a failure.
pub(super) fn entry_map(entry: &ManifestValue) -> Result<&ManifestMap, TestCaseError> {
    entry
        .as_object()
        .ok_or_else(|| TestCaseError::fail(format!("expanded entry is not an object: {entry}")))
}

/// Collect one map's key sequence in order.
pub(super) fn key_sequence(map: &ManifestMap) -> Vec<&str> {
    map.keys().map(String::as_str).collect()
}

/// Collect one map's key sequence as owned strings.
pub(super) fn owned_key_sequence(map: &ManifestMap) -> Vec<String> {
    map.keys().cloned().collect()
}

/// Read one entry's `vars` object, when it carries one.
pub(super) fn vars_of(entry: &ManifestValue) -> Option<&ManifestMap> {
    entry.get("vars").and_then(ManifestValue::as_object)
}
