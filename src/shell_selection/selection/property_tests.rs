//! Exercise selector decoding and serialization over bounded recursive values.

use super::ShellSelection;
use crate::arbitrary_json_test_support::{json_kind, json_values};
use crate::shell_selection::{ShellName, name::property_tests};
use pretty_assertions::assert_eq;
use proptest::{prelude::*, strategy::ValueTree, test_runner::TestRunner};
use regex::Regex;
use serde_json::Value;
use std::collections::BTreeSet;

/// Build the shared JSON strategy with shell-name strings as its string leaf.
fn selection_json_values() -> impl Strategy<Value = Value> {
    json_values(property_tests::arbitrary_names().boxed())
}

/// Build the independent acceptance oracle for a selector value.
///
/// # Errors
///
/// Report a disagreement when the regular-expression oracle accepts a name
/// that the production name parser rejects.
fn expected_selection(
    value: &Value,
    regex: &Regex,
) -> Result<Option<ShellSelection>, crate::shell_selection::ShellNameError> {
    match value {
        Value::Bool(false) => Ok(Some(ShellSelection::Direct)),
        Value::Bool(true) => Ok(Some(ShellSelection::PlatformDefault)),
        Value::String(text) if property_tests::oracle(regex, text) => {
            ShellName::parse(text).map(ShellSelection::Named).map(Some)
        }
        _ => Ok(None),
    }
}

proptest! {
    #[test]
    fn decoding_matches_the_independent_oracle(value in selection_json_values()) {
        let regex = Regex::new(r"\A[a-z][a-z0-9_-]{0,62}\z")
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let expected = expected_selection(&value, &regex)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let actual = serde_json::from_value::<ShellSelection>(value);
        prop_assert_eq!(actual.ok(), expected);
    }

    #[test]
    fn selections_round_trip_through_json_and_yaml(selection in selection_values()) {
        let json = serde_json::to_string(&selection)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let yaml = serde_saphyr::to_string(&selection)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(serde_json::from_str::<ShellSelection>(&json).ok(), Some(selection.clone()));
        prop_assert_eq!(serde_saphyr::from_str::<ShellSelection>(&yaml).ok(), Some(selection));
    }
}

/// Generate every selector variant and a validated arbitrary name.
fn selection_values() -> impl Strategy<Value = ShellSelection> {
    prop_oneof![
        Just(ShellSelection::Direct),
        Just(ShellSelection::PlatformDefault),
        property_tests::valid_names().prop_filter_map("valid shell name", |text| {
            ShellName::parse(&text).ok().map(ShellSelection::Named)
        }),
    ]
}

#[test]
fn value_generator_reaches_every_json_kind() -> Result<(), String> {
    let mut runner = TestRunner::deterministic();
    let strategy = selection_json_values();
    let mut observed = BTreeSet::new();
    for _ in 0..1_000 {
        let value = strategy
            .new_tree(&mut runner)
            .map_err(|error| error.to_string())?
            .current();
        observed.insert(json_kind(&value));
    }
    assert_eq!(
        observed,
        BTreeSet::from(["array", "boolean", "null", "number", "object", "string"])
    );
    Ok(())
}

#[test]
fn selection_generator_reaches_every_variant() -> Result<(), String> {
    let mut runner = TestRunner::deterministic();
    let strategy = selection_values();
    let mut observed = BTreeSet::new();
    for _ in 0..1_000 {
        let selection = strategy
            .new_tree(&mut runner)
            .map_err(|error| error.to_string())?
            .current();
        let variant = match selection {
            ShellSelection::Direct => "direct",
            ShellSelection::PlatformDefault => "platform-default",
            ShellSelection::Named(_) => "named",
        };
        observed.insert(variant);
    }
    assert_eq!(
        observed,
        BTreeSet::from(["direct", "named", "platform-default"])
    );
    Ok(())
}

#[rstest::rstest]
#[case("null")]
#[case("nan")]
#[case("inf")]
#[case("yess")]
#[case("no-op")]
fn scalar_like_names_round_trip_through_selection(#[case] text: &str) {
    let selection =
        ShellSelection::Named(ShellName::parse(text).expect("scalar-like name must parse"));
    let json = serde_json::to_string(&selection).expect("JSON selection must serialize");
    let yaml = serde_saphyr::to_string(&selection).expect("YAML selection must serialize");
    assert_eq!(
        serde_json::from_str::<ShellSelection>(&json).expect("JSON selection must decode"),
        selection
    );
    assert_eq!(
        serde_saphyr::from_str::<ShellSelection>(&yaml).expect("YAML selection must decode"),
        selection
    );
}
