//! Exercise selector decoding and serialization over bounded recursive values.

use super::ShellSelection;
use crate::shell_selection::{ShellName, name::property_tests};
use pretty_assertions::assert_eq;
use proptest::{prelude::*, strategy::ValueTree, test_runner::TestRunner};
use regex::Regex;
use serde_json::{Map, Number, Value};
use std::collections::BTreeSet;

/// Generate every JSON value kind with bounded recursive containers.
fn json_values() -> impl Strategy<Value = Value> {
    let leaves = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|number| Value::Number(Number::from(number))),
        (-10_000.0f64..10_000.0f64).prop_filter_map("finite JSON number", |number| {
            Number::from_f64(number).map(Value::Number)
        }),
        property_tests::arbitrary_names().prop_map(Value::String),
    ];
    let nested = leaves.clone().prop_recursive(2, 64, 4, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..=4).prop_map(Value::Array),
            prop::collection::btree_map("[a-z]{0,4}", inner, 0..=4)
                .prop_map(|entries| { Value::Object(Map::from_iter(entries)) }),
        ]
    });
    let containers = prop_oneof![
        Just(Value::Array(Vec::new())),
        Just(Value::Object(Map::new())),
    ];
    prop_oneof![5 => leaves, 2 => containers, 3 => nested]
}

/// Describe one of JSON's six value kinds for deterministic coverage checks.
fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
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
    fn decoding_matches_the_independent_oracle(value in json_values()) {
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
    let strategy = json_values();
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
