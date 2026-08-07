//! Tests for the workspace fallback environment switch.
//!
//! These construct [`WorkspaceSwitch`] states directly rather than setting
//! `NETSUKE_WHICH_WORKSPACE`, so they mutate nothing and run concurrently. The
//! non-UTF-8 branch is reachable only this way: fabricating such a value in the
//! live environment needs platform-specific `OsString` surgery, and under the
//! AGENTS.md testing mandate in-process mutation is not available regardless.
//!
//! The translation from a platform reading is exercised here too, even though
//! the `From` implementation lives with the adapter in `env`, because the
//! mapping is what pins these three states to the environment they model.

use super::{WORKSPACE_FALLBACK_ENV, WorkspaceSwitch};
use crate::test_tracing_capture::with_test_subscriber;
use rstest::rstest;
use std::env::VarError;
use std::ffi::OsString;
use tracing::level_filters::LevelFilter;

/// Values that switch the fallback off, in each accepted spelling and case.
#[rstest]
#[case("0")]
#[case("false")]
#[case("off")]
#[case("FALSE")]
#[case("Off")]
#[case("OFF")]
fn disabling_values_switch_the_fallback_off(#[case] value: &str) {
    assert!(
        !WorkspaceSwitch::Value(value.to_owned()).enabled(),
        "{value:?} should disable the workspace fallback"
    );
}

/// Anything else leaves the fallback on, including values that merely contain
/// a disabling word.
#[rstest]
#[case("1")]
#[case("true")]
#[case("on")]
#[case("")]
#[case("offbeat")]
#[case("no")]
fn other_values_leave_the_fallback_on(#[case] value: &str) {
    assert!(
        WorkspaceSwitch::Value(value.to_owned()).enabled(),
        "{value:?} should leave the workspace fallback enabled"
    );
}

#[test]
fn absent_variable_leaves_the_fallback_on() {
    assert!(
        WorkspaceSwitch::Absent.enabled(),
        "the fallback is opt-out, so an unset variable must leave it enabled"
    );
}

#[test]
fn non_utf8_value_switches_the_fallback_off() {
    assert!(
        !WorkspaceSwitch::NotUnicode.enabled(),
        "a non-UTF-8 value must disable the fallback rather than be treated as absent"
    );
}

/// The adapter maps each platform reading onto the state that models it.
#[rstest]
#[case(Ok(String::from("off")), WorkspaceSwitch::Value(String::from("off")))]
#[case(Err(VarError::NotPresent), WorkspaceSwitch::Absent)]
#[case(
    Err(VarError::NotUnicode(OsString::from("ignored"))),
    WorkspaceSwitch::NotUnicode
)]
fn readings_translate_to_switch_states(
    #[case] raw: Result<String, VarError>,
    #[case] expected: WorkspaceSwitch,
) {
    assert_eq!(WorkspaceSwitch::from(raw), expected);
}

/// The seam must name the documented variable, not some other one.
///
/// That the capture boundary actually asks for this key is pinned separately
/// by the `MockEnv` expectations in the `env` capture tests.
#[test]
fn the_documented_variable_name_is_used() {
    assert_eq!(WORKSPACE_FALLBACK_ENV, "NETSUKE_WHICH_WORKSPACE");
}

/// The decision is pure: consulting the switch must emit no events.
///
/// The non-UTF-8 diagnostic fires once at the capture boundary —
/// `EnvSnapshot::capture_with_env` warns immediately after the raw read; see
/// the capture-level test in `env.rs` — so the state itself must stay silent
/// however often it is consulted.
#[test]
fn non_utf8_classification_is_silent() {
    let (enabled, events) = with_test_subscriber(LevelFilter::WARN, |captured| {
        let enabled = WorkspaceSwitch::NotUnicode.enabled();
        (enabled, captured.snapshot())
    });

    assert!(!enabled, "a non-UTF-8 value must disable the fallback");
    assert!(
        events.is_empty(),
        "consulting the switch must emit no events, got {events:?}"
    );
}

mod properties {
    //! Property coverage for the fallback switch.
    //!
    //! The fixed cases name the three disabling spellings; these state the
    //! classification rule over arbitrary values, using an independent model
    //! rather than re-deriving the implementation's own `matches!`.

    use super::WorkspaceSwitch;
    use proptest::prelude::*;

    /// The rule, written independently: exactly three values disable it, and
    /// case is ignored. Deliberately a set lookup rather than the same
    /// `matches!` the implementation uses, so a change to one does not
    /// silently change the other.
    fn model_says_enabled(value: &str) -> bool {
        const DISABLING: [&str; 3] = ["0", "false", "off"];
        !DISABLING.contains(&value.to_ascii_lowercase().as_str())
    }

    /// Every case permutation of one disabling word.
    ///
    /// Per-character case flags rather than a fixed spelling list: a
    /// hardcoded handful of variants cannot catch a normalizer that
    /// special-cases exactly those spellings, whereas this reaches all
    /// 2^5 + 2^3 + 1 permutations of `false`, `off`, and `0`.
    fn case_variant() -> impl Strategy<Value = String> {
        prop_oneof![
            1 => Just(String::from("0")),
            4 => mixed_case("false"),
            3 => mixed_case("off"),
        ]
    }

    /// `word` with each character independently upper- or lowercased.
    fn mixed_case(word: &'static str) -> impl Strategy<Value = String> {
        proptest::collection::vec(any::<bool>(), word.len()).prop_map(move |uppers| {
            word.chars()
                .zip(uppers)
                .map(|(ch, upper)| if upper { ch.to_ascii_uppercase() } else { ch })
                .collect()
        })
    }

    proptest! {
        /// Classification matches the model for any value, however spelled.
        #[test]
        fn classification_matches_the_model(value in "\\PC{0,12}") {
            let expected = model_says_enabled(&value);
            let diagnostic = format!("value {value:?}");
            let enabled = WorkspaceSwitch::Value(value).enabled();
            prop_assert_eq!(enabled, expected, "{}", diagnostic);
        }

        /// Every case variant of a disabling word disables the fallback.
        ///
        /// Generated separately because a uniform string generator produces
        /// `"off"` about never, so the interesting half of the rule would go
        /// untested by the property above alone.
        #[test]
        fn case_variants_disable(value in case_variant()) {
            let diagnostic = format!("{value:?} should disable the fallback");
            prop_assert!(!WorkspaceSwitch::Value(value).enabled(), "{}", diagnostic);
        }

        /// Anything outside the disabling set enables it.
        #[test]
        fn other_values_enable(value in "\\PC{0,12}".prop_filter(
            "must not be a disabling spelling",
            |v: &String| !matches!(v.to_ascii_lowercase().as_str(), "0" | "false" | "off"),
        )) {
            prop_assert!(WorkspaceSwitch::Value(value).enabled());
        }
    }

    /// An absent variable enables the fallback; a non-UTF-8 one disables it.
    ///
    /// Fixed rather than generated: both are single states with no payload to
    /// explore.
    #[test]
    fn error_states_are_classified() {
        assert!(WorkspaceSwitch::Absent.enabled());
        assert!(!WorkspaceSwitch::NotUnicode.enabled());
    }
}
