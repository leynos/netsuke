//! Direct coverage for the `POLONIUS_FLAGS` resolution guard.
//!
//! The `RUSTFLAGS` contract assertions all use `contains`, which an empty
//! string satisfies vacuously, so `rustflags::polonius_flags` must reject a
//! missing or empty definition rather than hand back a value that voids the
//! Polonius contract. These tests exercise the guard against synthetic
//! Makefile text — the behavioural tests only ever see the real, non-empty
//! definition — and a bounded property test pins the acceptance invariant:
//! resolution succeeds exactly when a definition exists whose value is
//! non-empty after trimming.

use super::rustflags::polonius_flags;
use anyhow::{Result, ensure};
use proptest::prelude::*;
use rstest::rstest;

#[rstest]
#[case::defined_with_flag("POLONIUS_FLAGS ?= -Zpolonius=next\n", Some("-Zpolonius=next"))]
#[case::defined_with_plain_assignment(
    "POLONIUS_FLAGS = -Zpolonius=next\n",
    Some("-Zpolonius=next")
)]
#[case::missing_definition("OTHER_VAR ?= value\n", None)]
#[case::empty_value("POLONIUS_FLAGS ?= \n", None)]
#[case::whitespace_only_value("POLONIUS_FLAGS ?= \t \n", None)]
fn polonius_flags_accepts_only_non_empty_definitions(
    #[case] makefile: &str,
    #[case] expected: Option<&str>,
) -> Result<()> {
    let outcome = polonius_flags(makefile);
    if let Some(value) = expected {
        let resolved = outcome?;
        ensure!(
            resolved == value,
            "expected {value:?} from {makefile:?}; got {resolved:?}"
        );
    } else {
        let error = outcome.err().map(|error| error.to_string());
        let message = error.as_deref().unwrap_or("(unexpectedly succeeded)");
        ensure!(
            message.contains("POLONIUS_FLAGS"),
            "rejection for {makefile:?} should name the variable; got {message:?}"
        );
    }
    Ok(())
}

#[rstest]
fn polonius_flags_names_the_missing_variable() {
    let error = polonius_flags("A ?= b\n").expect_err("missing definition should fail");
    assert!(
        error.to_string().contains("should define POLONIUS_FLAGS"),
        "missing definition should report the expected message; got {error:?}"
    );
}

#[rstest]
fn polonius_flags_names_the_empty_value() {
    let error = polonius_flags("POLONIUS_FLAGS ?=  \n").expect_err("empty definition should fail");
    assert!(
        error.to_string().contains("should not be empty"),
        "empty definition should report the expected message; got {error:?}"
    );
}

/// Strategy for the `POLONIUS_FLAGS` value slot: absent, whitespace-only, or
/// a non-empty flag-like token.
fn arb_polonius_value() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        Just(None),
        "[ \t]{0,4}".prop_map(Some),
        "[-A-Za-z0-9=+.]{1,24}".prop_map(Some),
    ]
}

proptest! {
    /// Resolution succeeds exactly when a definition exists whose value is
    /// non-empty after trimming, and the resolved text is the trimmed value.
    #[test]
    fn prop_polonius_flags_acceptance_matches_the_definition(
        value in arb_polonius_value(),
        filler in "[A-Z_]{1,8}",
    ) {
        let makefile = value.as_ref().map_or_else(
            || format!("{filler} ?= x\n"),
            |text| format!("{filler} ?= x\nPOLONIUS_FLAGS ?= {text}\n"),
        );
        let outcome = polonius_flags(&makefile);
        let expected = value
            .as_deref()
            .map(str::trim)
            .filter(|trimmed| !trimmed.is_empty());
        if let Some(trimmed) = expected {
            let resolved = outcome.map_err(|error| TestCaseError::fail(error.to_string()))?;
            prop_assert_eq!(resolved, trimmed, "resolved value should be the trimmed text");
        } else {
            prop_assert!(
                outcome.is_err(),
                "missing or blank definitions must be rejected: {makefile:?}"
            );
        }
    }
}
