//! Tests for strict Boolean environment spellings.
//!
//! Only the five `CliConfig` Boolean paths accept `1` and `0`; configuration
//! files keep strict Boolean typing. These tests pin the per-entry decision and
//! prove that non-Boolean paths keep Figment's ordinary scalar parsing.

use super::super::{BOOLEAN_PATHS, EnvironmentLayer, Value, parse_entry};
use ortho_config::figment::{Profile, Provider};
use rstest::rstest;
use std::ffi::{OsStr, OsString};

/// Interpret one environment entry through the real `parse_entry` seam.
fn parse(key: &str, value: &str) -> Value {
    let parsed = parse_entry(OsStr::new(key), OsStr::new(value))
        .unwrap_or_else(|error| panic!("{key:?} should parse: {error}"));
    let Some((_, parsed_value)) = parsed else {
        panic!("{key:?} should be a Netsuke configuration key");
    };
    parsed_value
}

/// Every Boolean path accepts exactly the four strict spellings.
#[rstest]
#[case::true_spelling("true", true)]
#[case::false_spelling("false", false)]
#[case::one("1", true)]
#[case::zero("0", false)]
fn boolean_paths_accept_strict_spellings(#[case] raw: &str, #[case] expected: bool) {
    for path in BOOLEAN_PATHS {
        let key = format!("NETSUKE_{}", path.to_ascii_uppercase());
        let parsed = parse(&key, raw);
        assert_eq!(
            parsed,
            Value::from(expected),
            "{key}={raw} should parse as Boolean {expected}",
        );
    }
}

/// Case, whitespace, and non-Boolean text stay raw strings on Boolean paths.
#[rstest]
#[case::capitalized_true("True")]
#[case::upper_true("TRUE")]
#[case::yes("yes")]
#[case::two("2")]
#[case::empty("")]
#[case::surrounded_by_spaces(" 1 ")]
#[case::trailing_space("1 ")]
fn boolean_paths_keep_rejected_spellings_as_strings(#[case] raw: &str) {
    for path in BOOLEAN_PATHS {
        let key = format!("NETSUKE_{}", path.to_ascii_uppercase());
        let parsed = parse(&key, raw);
        assert_eq!(
            parsed.as_str(),
            Some(raw),
            "{key}={raw:?} should remain a raw string for the merge to reject",
        );
    }
}

/// Normalized case and key whitespace still select a Boolean path.
#[rstest]
#[case::lower_case("netsuke_json")]
#[case::surrounding_spaces(" NETSUKE_JSON ")]
fn normalized_aliases_select_the_boolean_path(#[case] key: &str) {
    assert_eq!(
        parse(key, "1"),
        Value::from(true),
        "{key:?} should normalize to the json Boolean path",
    );
}

/// Non-Boolean paths keep Figment's number and array parsing.
#[test]
fn non_boolean_paths_keep_figment_scalar_parsing() {
    assert!(
        matches!(parse("NETSUKE_JOBS", "1"), Value::Num(..)),
        "jobs should remain a number rather than becoming a Boolean",
    );
    assert_eq!(
        parse("NETSUKE_DEFAULT_TARGETS", "hello.txt").as_str(),
        Some("hello.txt"),
        "a scalar list value should remain a string",
    );
    assert!(
        matches!(
            parse("NETSUKE_DEFAULT_TARGETS", r#"["hello.txt"]"#),
            Value::Array(..)
        ),
        "a serialized JSON array should remain an array",
    );
}

/// Non-Boolean scalar strings are trimmed by Figment's text parser.
///
/// This asymmetry with a quoted configuration-file value is deliberate and
/// documented: the trim belongs to the environment value parser, not to the
/// policy contract.
#[test]
fn non_boolean_string_values_are_trimmed() {
    assert_eq!(
        parse("NETSUKE_COLOR", " always ").as_str(),
        Some("always"),
        "an unquoted environment scalar should be trimmed before policy parsing",
    );
}

/// A nested path that ends in a Boolean name is not the Boolean field itself.
#[test]
fn nested_boolean_named_paths_are_not_normalized() {
    assert_eq!(
        parse("NETSUKE_CMDS__JSON", "1").to_u128(),
        Some(1),
        "a nested json path should keep its numeric value",
    );
}

/// The provider emits Booleans only for the named Boolean paths.
#[test]
fn provider_emits_booleans_only_for_boolean_paths() {
    let layer = EnvironmentLayer::new(vec![
        (OsString::from("NETSUKE_JSON"), OsString::from("1")),
        (OsString::from("NETSUKE_JOBS"), OsString::from("1")),
        (OsString::from("NETSUKE_VERBOSE"), OsString::from("yes")),
    ]);
    let data = layer
        .data()
        .expect("valid entries should produce provider data");
    let defaults = data
        .get(&Profile::Default)
        .expect("provider should emit the default profile");

    assert_eq!(
        defaults.get("json"),
        Some(&Value::from(true)),
        "json should be emitted as a Boolean",
    );
    assert_eq!(
        defaults.get("jobs").and_then(Value::to_u128),
        Some(1),
        "jobs should keep its numeric value",
    );
    assert_eq!(
        defaults.get("verbose").and_then(Value::as_str),
        Some("yes"),
        "a rejected Boolean spelling should remain a raw string",
    );
}
