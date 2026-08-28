//! Tests for the domain configuration policy enums.
//!
//! Pins the case-insensitive `FromStr` contract that replaced
//! `clap::ValueEnum`: every accepted spelling parses from upper-case,
//! lower-case, and mixed-case input, and invalid values are rejected with the
//! policy-specific error.

use super::*;
use rstest::rstest;

/// One accepted spelling plus the variant it must parse to.
struct ParseCase<'a, T> {
    /// Raw input handed to `str::parse`.
    input: &'a str,
    /// Variant expected back from the parse.
    expected: T,
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
