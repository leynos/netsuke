//! Tests for the domain configuration policy enums.
//!
//! Pins the case-insensitive `FromStr` and `Deserialize` contracts that
//! replaced `clap::ValueEnum`: every accepted spelling parses from upper-case,
//! lower-case, and mixed-case input through both string parsing and the TOML
//! and JSON configuration layers. Serialization stays canonical lowercase, and
//! invalid values carry the policy-specific error.

use super::policy_definitions::{
    ACCESSIBILITY_POLICY_DEFINITIONS, COLOUR_POLICY_DEFINITIONS, EMOJI_POLICY_DEFINITIONS,
    PROGRESS_POLICY_DEFINITIONS, PolicyDefinition,
};
use super::*;
use proptest::prelude::*;
use rstest::rstest;
use serde::de::DeserializeOwned;
use serde_json::json;

/// One-field wrapper so a policy can be deserialized from TOML.
#[derive(Debug, serde::Deserialize)]
struct PolicyField<T> {
    value: T,
}

/// Prove one spelling deserializes to `expected` through JSON and TOML.
fn assert_policy_accepts<T>(spelling: &str, expected: &T)
where
    T: DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let from_json: T = match serde_json::from_value(serde_json::Value::from(spelling)) {
        Ok(value) => value,
        Err(error) => panic!("{spelling:?} should deserialize from JSON: {error}"),
    };
    assert_eq!(&from_json, expected, "JSON spelling {spelling:?}");

    let from_toml: PolicyField<T> = match toml::from_str(&format!("value = {spelling:?}\n")) {
        Ok(value) => value,
        Err(error) => panic!("{spelling:?} should deserialize from TOML: {error}"),
    };
    assert_eq!(&from_toml.value, expected, "TOML spelling {spelling:?}");
}

/// One accepted spelling plus the variant it must parse to.
struct ParseCase<'a, T> {
    /// Raw input handed to `str::parse`.
    input: &'a str,
    /// Variant expected back from the parse.
    expected: T,
}

/// Generate one accepted spelling with every ASCII letter independently cased.
fn policy_case_strategy<T: Copy + std::fmt::Debug + 'static>(
    definitions: &'static [PolicyDefinition<T>],
) -> impl Strategy<Value = (String, T)> {
    prop::sample::select(definitions.to_vec()).prop_flat_map(|definition| {
        prop::collection::vec(any::<bool>(), definition.spelling.len()).prop_map(move |cases| {
            (
                with_ascii_case(definition.spelling, cases),
                definition.variant,
            )
        })
    })
}

/// Apply the independently generated ASCII casing choices to `spelling`.
fn with_ascii_case(spelling: &str, cases: Vec<bool>) -> String {
    spelling
        .chars()
        .zip(cases)
        .map(|(character, upper)| ascii_case(character, upper))
        .collect()
}

/// Select the requested ASCII case for one policy spelling character.
fn ascii_case(character: char, upper: bool) -> char {
    if upper {
        character.to_ascii_uppercase()
    } else {
        character
    }
}

/// `ColourPolicy` accepts `auto`, `always`, and `never` in any casing.
#[rstest]
#[case::lower_case("auto", "always", "never")]
#[case::upper_case("AUTO", "ALWAYS", "NEVER")]
#[case::mixed_case("Auto", "AlWaYs", "NeVeR")]
fn colour_policy_parses_case_insensitively(
    #[case] auto: &str,
    #[case] always: &str,
    #[case] never: &str,
) {
    let cases = [
        ParseCase {
            input: auto,
            expected: ColourPolicy::Auto,
        },
        ParseCase {
            input: always,
            expected: ColourPolicy::Always,
        },
        ParseCase {
            input: never,
            expected: ColourPolicy::Never,
        },
    ];
    for case in &cases {
        let parsed: ColourPolicy = case.input.parse().expect("input should parse");
        assert_eq!(parsed, case.expected, "input '{}'", case.input);
        assert_policy_accepts(case.input, &case.expected);
    }
}

/// `EmojiPolicy` accepts `auto`, `always`, and `never` in any casing.
#[rstest]
#[case::lower_case("auto", "always", "never")]
#[case::upper_case("AUTO", "ALWAYS", "NEVER")]
#[case::mixed_case("AuTo", "aLWAYS", "NEveR")]
fn emoji_policy_parses_case_insensitively(
    #[case] auto: &str,
    #[case] always: &str,
    #[case] never: &str,
) {
    let cases = [
        ParseCase {
            input: auto,
            expected: EmojiPolicy::Auto,
        },
        ParseCase {
            input: always,
            expected: EmojiPolicy::Always,
        },
        ParseCase {
            input: never,
            expected: EmojiPolicy::Never,
        },
    ];
    for case in &cases {
        let parsed: EmojiPolicy = case.input.parse().expect("input should parse");
        assert_eq!(parsed, case.expected, "input '{}'", case.input);
        assert_policy_accepts(case.input, &case.expected);
    }
}

/// `ProgressPolicy` accepts `auto`, `always`, and `never` in any casing.
#[rstest]
#[case::lower_case("auto", "always", "never")]
#[case::upper_case("AUTO", "ALWAYS", "NEVER")]
#[case::mixed_case("aUtO", "Always", "nEVER")]
fn progress_policy_parses_case_insensitively(
    #[case] auto: &str,
    #[case] always: &str,
    #[case] never: &str,
) {
    let cases = [
        ParseCase {
            input: auto,
            expected: ProgressPolicy::Auto,
        },
        ParseCase {
            input: always,
            expected: ProgressPolicy::Always,
        },
        ParseCase {
            input: never,
            expected: ProgressPolicy::Never,
        },
    ];
    for case in &cases {
        let parsed: ProgressPolicy = case.input.parse().expect("input should parse");
        assert_eq!(parsed, case.expected, "input '{}'", case.input);
        assert_policy_accepts(case.input, &case.expected);
    }
}

/// `AccessibilityPolicy` accepts `auto`, `on`, and `off` in any casing.
#[rstest]
#[case::lower_case("auto", "on", "off")]
#[case::upper_case("AUTO", "ON", "OFF")]
#[case::mixed_case("AUto", "oN", "OfF")]
fn accessibility_policy_parses_case_insensitively(
    #[case] auto: &str,
    #[case] on: &str,
    #[case] off: &str,
) {
    let cases = [
        ParseCase {
            input: auto,
            expected: AccessibilityPolicy::Auto,
        },
        ParseCase {
            input: on,
            expected: AccessibilityPolicy::On,
        },
        ParseCase {
            input: off,
            expected: AccessibilityPolicy::Off,
        },
    ];
    for case in &cases {
        let parsed: AccessibilityPolicy = case.input.parse().expect("input should parse");
        assert_eq!(parsed, case.expected, "input '{}'", case.input);
        assert_policy_accepts(case.input, &case.expected);
    }
}

/// Each policy rejects unknown values with its own error message.
#[test]
fn policy_parse_rejects_unknown_values() {
    assert_eq!(
        "bogus".parse::<ColourPolicy>().expect_err("should reject"),
        "invalid color policy 'bogus'"
    );
    assert_eq!(
        "bogus".parse::<EmojiPolicy>().expect_err("should reject"),
        "invalid emoji policy 'bogus'"
    );
    assert_eq!(
        "bogus"
            .parse::<ProgressPolicy>()
            .expect_err("should reject"),
        "invalid progress policy 'bogus'"
    );
    assert_eq!(
        "bogus"
            .parse::<AccessibilityPolicy>()
            .expect_err("should reject"),
        "invalid accessibility policy 'bogus'"
    );
}

/// Assert that `value` serializes to the canonical `expected` spelling.
fn assert_canonical_spelling<T: serde::Serialize>(value: T, expected: &str) {
    let serialized = serde_json::to_value(value).expect("policy should serialize");
    assert_eq!(
        serialized,
        json!(expected),
        "policy should serialize as {expected:?}",
    );
}

/// Every variant serializes with its canonical lowercase spelling.
#[test]
fn policies_serialize_with_canonical_spellings() {
    assert_canonical_spelling(ColourPolicy::Auto, "auto");
    assert_canonical_spelling(ColourPolicy::Always, "always");
    assert_canonical_spelling(ColourPolicy::Never, "never");
    assert_canonical_spelling(EmojiPolicy::Auto, "auto");
    assert_canonical_spelling(EmojiPolicy::Always, "always");
    assert_canonical_spelling(EmojiPolicy::Never, "never");
    assert_canonical_spelling(ProgressPolicy::Auto, "auto");
    assert_canonical_spelling(ProgressPolicy::Always, "always");
    assert_canonical_spelling(ProgressPolicy::Never, "never");
    assert_canonical_spelling(AccessibilityPolicy::Auto, "auto");
    assert_canonical_spelling(AccessibilityPolicy::On, "on");
    assert_canonical_spelling(AccessibilityPolicy::Off, "off");
}

/// Serde failures keep the policy error and name the accepted spellings.
#[test]
fn policies_reject_unknown_serde_values_with_expected_spellings() {
    let colour = serde_json::from_value::<ColourPolicy>(json!("bogus"))
        .expect_err("an unknown colour policy must be rejected");
    assert!(
        colour.to_string().contains("invalid color policy 'bogus'"),
        "expected the FromStr error text, got: {colour}",
    );
    assert!(
        colour
            .to_string()
            .contains("expected one of: auto, always, never"),
        "expected the canonical spellings, got: {colour}",
    );

    let emoji = serde_json::from_value::<EmojiPolicy>(json!("bogus"))
        .expect_err("an unknown emoji policy must be rejected");
    assert!(
        emoji.to_string().contains("invalid emoji policy 'bogus'"),
        "expected the FromStr error text, got: {emoji}",
    );

    let progress = serde_json::from_value::<ProgressPolicy>(json!(1))
        .expect_err("a non-string progress policy must be rejected");
    assert!(
        progress.to_string().contains("invalid type"),
        "expected an invalid-type rejection, got: {progress}",
    );

    let accessibility = toml::from_str::<PolicyField<AccessibilityPolicy>>("value = \"bogus\"\n")
        .expect_err("an unknown accessibility policy must be rejected");
    assert!(
        accessibility
            .to_string()
            .contains("invalid accessibility policy 'bogus'"),
        "expected the FromStr error text from TOML, got: {accessibility}",
    );
}

proptest! {
    #[test]
    fn colour_policy_accepts_all_ascii_case_variants(
        (input, expected) in policy_case_strategy(&COLOUR_POLICY_DEFINITIONS)
    ) {
        prop_assert_eq!(input.parse::<ColourPolicy>(), Ok(expected));
        let from_json: ColourPolicy =
            serde_json::from_value(serde_json::Value::from(input.as_str()))
                .expect("generated colour spelling should deserialize");
        prop_assert_eq!(from_json, expected);
    }

    #[test]
    fn emoji_policy_accepts_all_ascii_case_variants(
        (input, expected) in policy_case_strategy(&EMOJI_POLICY_DEFINITIONS)
    ) {
        prop_assert_eq!(input.parse::<EmojiPolicy>(), Ok(expected));
        let from_json: EmojiPolicy =
            serde_json::from_value(serde_json::Value::from(input.as_str()))
                .expect("generated emoji spelling should deserialize");
        prop_assert_eq!(from_json, expected);
    }

    #[test]
    fn progress_policy_accepts_all_ascii_case_variants(
        (input, expected) in policy_case_strategy(&PROGRESS_POLICY_DEFINITIONS)
    ) {
        prop_assert_eq!(input.parse::<ProgressPolicy>(), Ok(expected));
        let from_json: ProgressPolicy =
            serde_json::from_value(serde_json::Value::from(input.as_str()))
                .expect("generated progress spelling should deserialize");
        prop_assert_eq!(from_json, expected);
    }

    #[test]
    fn accessibility_policy_accepts_all_ascii_case_variants(
        (input, expected) in policy_case_strategy(&ACCESSIBILITY_POLICY_DEFINITIONS)
    ) {
        prop_assert_eq!(input.parse::<AccessibilityPolicy>(), Ok(expected));
        let from_json: AccessibilityPolicy =
            serde_json::from_value(serde_json::Value::from(input.as_str()))
                .expect("generated accessibility spelling should deserialize");
        prop_assert_eq!(from_json, expected);
    }
}
