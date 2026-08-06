//! Tests for the workspace fallback environment switch.
//!
//! These drive `workspace_fallback_enabled_with` directly rather than setting
//! `NETSUKE_WHICH_WORKSPACE`, so they mutate nothing and run concurrently. The
//! non-UTF-8 branch is reachable only this way: fabricating such a value in the
//! live environment needs platform-specific `OsString` surgery, and under the
//! AGENTS.md testing mandate in-process mutation is not available regardless.

use super::{WORKSPACE_FALLBACK_ENV, workspace_fallback_enabled_with};
use rstest::rstest;

/// The exact warning `workspace_fallback_enabled_with` emits for a non-UTF-8
/// value. Asserted verbatim: without it the test passes even when the message
/// is replaced wholesale, which is precisely what happened before.
const NOT_UNICODE_WARNING: &str = "workspace fallback disabled because env var is not valid UTF-8";
use crate::test_tracing_capture::with_test_subscriber;
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
        !workspace_fallback_enabled_with(|_| Ok(value.to_owned())),
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
        workspace_fallback_enabled_with(|_| Ok(value.to_owned())),
        "{value:?} should leave the workspace fallback enabled"
    );
}

#[test]
fn absent_variable_leaves_the_fallback_on() {
    assert!(
        workspace_fallback_enabled_with(|_| Err(VarError::NotPresent)),
        "the fallback is opt-out, so an unset variable must leave it enabled"
    );
}

#[test]
fn non_utf8_value_switches_the_fallback_off() {
    let err = VarError::NotUnicode(OsString::from("ignored"));
    assert!(
        !workspace_fallback_enabled_with(|_| Err(err)),
        "a non-UTF-8 value must disable the fallback rather than be treated as absent"
    );
}

/// The seam must consult the documented variable, not some other name.
#[test]
fn the_documented_variable_name_is_consulted() {
    let mut observed = None;
    let _ = workspace_fallback_enabled_with(|key| {
        observed = Some(key.to_owned());
        Err(VarError::NotPresent)
    });
    assert_eq!(observed.as_deref(), Some(WORKSPACE_FALLBACK_ENV));
}

/// The non-UTF-8 branch must warn, not merely disable the fallback.
///
/// Silently switching workspace search off would leave a user whose `PATHEXT`-
/// style variable is mis-encoded with commands mysteriously unresolved and no
/// indication why. The boolean assertion alone would not notice the warning
/// being deleted.
#[test]
fn non_utf8_value_warns_before_disabling() {
    let err = VarError::NotUnicode(OsString::from("ignored"));
    let (enabled, events) = with_test_subscriber(LevelFilter::WARN, |captured| {
        let enabled = workspace_fallback_enabled_with(|_| Err(err));
        (enabled, captured.snapshot())
    });

    assert!(!enabled, "a non-UTF-8 value must disable the fallback");
    // The subscriber admits WARN and above, so the level is asserted by the
    // filter rather than by a string prefix; requiring exactly one event keeps
    // that as strict as the previous `starts_with("WARN")` check.
    assert_eq!(
        events.len(),
        1,
        "expected exactly one warning, got {events:?}"
    );
    assert!(
        events.iter().any(|event| {
            event.contains(WORKSPACE_FALLBACK_ENV) && event.contains(NOT_UNICODE_WARNING)
        }),
        concat!(
            "expected a warning naming {} and carrying {:?}, ",
            "got {:?}"
        ),
        WORKSPACE_FALLBACK_ENV,
        NOT_UNICODE_WARNING,
        events
    );
}

mod properties {
    //! Property coverage for the fallback switch.
    //!
    //! The fixed cases name the three disabling spellings; these state the
    //! classification rule over arbitrary values, using an independent model
    //! rather than re-deriving the implementation's own `matches!`.

    use super::{WORKSPACE_FALLBACK_ENV, workspace_fallback_enabled_with};
    use proptest::prelude::*;
    use std::env::VarError;

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
            let enabled = workspace_fallback_enabled_with(move |_| Ok(value));
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
            prop_assert!(
                !workspace_fallback_enabled_with(move |_| Ok(value)),
                "{}", diagnostic
            );
        }

        /// Anything outside the disabling set enables it.
        #[test]
        fn other_values_enable(value in "\\PC{0,12}".prop_filter(
            "must not be a disabling spelling",
            |v: &String| !matches!(v.to_ascii_lowercase().as_str(), "0" | "false" | "off"),
        )) {
            prop_assert!(workspace_fallback_enabled_with(move |_| Ok(value)));
        }

        /// The variable name asked for is always the documented one.
        #[test]
        fn the_queried_key_is_stable(value in "\\PC{0,8}") {
            let mut seen = None;
            let seen_slot = &mut seen;
            let _ = workspace_fallback_enabled_with(move |key| {
                *seen_slot = Some(key.to_owned());
                Ok(value)
            });
            prop_assert_eq!(seen.as_deref(), Some(WORKSPACE_FALLBACK_ENV));
        }
    }

    /// An absent variable enables the fallback; a non-UTF-8 one disables it.
    ///
    /// Fixed rather than generated: both are single states, and `VarError`
    /// has no other inhabitants to explore.
    #[test]
    fn error_variants_are_classified() {
        assert!(workspace_fallback_enabled_with(|_| Err(
            VarError::NotPresent
        )));
        assert!(!workspace_fallback_enabled_with(|_| Err(
            VarError::NotUnicode(std::ffi::OsString::from("x"))
        )));
    }
}
