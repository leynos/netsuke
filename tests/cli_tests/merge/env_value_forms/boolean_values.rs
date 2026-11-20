//! Strict Boolean spellings for the five Boolean configuration keys.

use super::*;
use rstest::rstest;

/// Every Boolean key accepts exactly `true`, `false`, `1`, and `0`.
///
/// The four environment-backed keys share one table. `no_input` has its own
/// pair of tests below because `0` merges successfully and is then rejected by
/// the no-input policy check, so it cannot share this success table.
#[rstest]
#[case::json("NETSUKE_JSON", |cli: &Cli| cli.json)]
#[case::verbose("NETSUKE_VERBOSE", |cli: &Cli| cli.verbose)]
#[case::fetch_default_deny("NETSUKE_FETCH_DEFAULT_DENY", |cli: &Cli| cli.fetch_default_deny)]
#[case::trust_project_fetch_policy(
    "NETSUKE_TRUST_PROJECT_FETCH_POLICY",
    |cli: &Cli| cli.trust_project_fetch_policy
)]
fn boolean_keys_accept_strict_spellings(
    #[case] key: &str,
    #[case] read: fn(&Cli) -> bool,
) -> Result<()> {
    for (raw, expected) in [("true", true), ("1", true), ("false", false), ("0", false)] {
        let merged = merge(&MergeCase::environment(&[(key, raw)]))?;
        let actual = read(&merged);
        ensure!(
            actual == expected,
            "{key}={raw:?} should merge as {expected}, got {actual}",
        );
    }
    Ok(())
}

/// `no_input = 1` parses as Boolean `true` rather than a number or string.
#[test]
fn no_input_accepts_one_as_true() -> Result<()> {
    let merged = merge(&MergeCase::environment(&[("NETSUKE_NO_INPUT", "1")]))?;
    ensure!(
        merged.interaction.no_input,
        "NETSUKE_NO_INPUT=1 should merge as true, got {}",
        merged.interaction.no_input,
    );
    Ok(())
}

/// `no_input = 0` parses as Boolean `false` and reaches the policy rejection.
///
/// The validation text proves the value crossed the merge as a Boolean: a raw
/// string would instead fail deserialization with an invalid-type error and
/// never reach the post-merge policy check.
#[test]
fn no_input_zero_reaches_the_validation_rejection() -> Result<()> {
    let error = merge_error(&MergeCase::environment(&[("NETSUKE_NO_INPUT", "0")]))?;
    ensure!(
        error.contains("no_input = false is unsupported"),
        "NETSUKE_NO_INPUT=0 should reach the no-input policy rejection, got: {error}",
    );
    Ok(())
}

/// Case, whitespace, and non-Boolean spellings are rejected on every key.
#[rstest]
#[case::capitalized_true("True")]
#[case::upper_true("TRUE")]
#[case::yes("yes")]
#[case::two("2")]
#[case::empty("")]
#[case::surrounded_by_spaces(" 1 ")]
#[case::trailing_space("1 ")]
fn rejected_spellings_fail_the_full_merge(#[case] raw: &str) -> Result<()> {
    for key in [
        "NETSUKE_VERBOSE",
        "NETSUKE_JSON",
        "NETSUKE_FETCH_DEFAULT_DENY",
        "NETSUKE_TRUST_PROJECT_FETCH_POLICY",
        "NETSUKE_NO_INPUT",
    ] {
        assert_environment_rejected(key, raw)?;
    }
    Ok(())
}

/// Configuration files keep strict Boolean typing, so `json = 1` is invalid.
#[test]
fn configuration_file_rejects_numeric_booleans() -> Result<()> {
    let error = merge_error(&MergeCase::file("json = 1\n"))?;
    ensure!(
        error.contains("invalid type"),
        "a numeric file Boolean should fail deserialization, got: {error}",
    );
    Ok(())
}

/// An accepted environment Boolean overrides the file's opposite value.
#[test]
fn environment_boolean_overrides_file_value() -> Result<()> {
    let merged =
        merge(&MergeCase::file("json = false\n").with_overrides(&[("NETSUKE_JSON", "1")]))?;
    ensure!(
        merged.json,
        "NETSUKE_JSON=1 should override the file's json = false",
    );
    Ok(())
}
