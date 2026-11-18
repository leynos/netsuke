//! Strict Boolean spellings for the five Boolean configuration keys.

use super::*;
use rstest::rstest;

/// `json` accepts exactly `true`, `false`, `1`, and `0` from the environment.
#[rstest]
#[case::true_spelling("true", true)]
#[case::one("1", true)]
#[case::false_spelling("false", false)]
#[case::zero("0", false)]
fn json_accepts_strict_spellings(#[case] raw: &str, #[case] expected: bool) -> Result<()> {
    let merged = merge_with_environment(&[("NETSUKE_JSON", raw)], &["netsuke"])?;
    ensure!(
        merged.json == expected,
        "NETSUKE_JSON={raw:?} should merge as {expected}, got {}",
        merged.json,
    );
    Ok(())
}

/// `verbose` accepts the same four spellings.
#[rstest]
#[case::true_spelling("true", true)]
#[case::one("1", true)]
#[case::false_spelling("false", false)]
#[case::zero("0", false)]
fn verbose_accepts_strict_spellings(#[case] raw: &str, #[case] expected: bool) -> Result<()> {
    let merged = merge_with_environment(&[("NETSUKE_VERBOSE", raw)], &["netsuke"])?;
    ensure!(
        merged.verbose == expected,
        "NETSUKE_VERBOSE={raw:?} should merge as {expected}, got {}",
        merged.verbose,
    );
    Ok(())
}

/// `fetch_default_deny` accepts the same four spellings.
#[rstest]
#[case::true_spelling("true", true)]
#[case::one("1", true)]
#[case::false_spelling("false", false)]
#[case::zero("0", false)]
fn fetch_default_deny_accepts_strict_spellings(
    #[case] raw: &str,
    #[case] expected: bool,
) -> Result<()> {
    let merged = merge_with_environment(&[("NETSUKE_FETCH_DEFAULT_DENY", raw)], &["netsuke"])?;
    ensure!(
        merged.fetch_default_deny == expected,
        "NETSUKE_FETCH_DEFAULT_DENY={raw:?} should merge as {expected}, got {}",
        merged.fetch_default_deny,
    );
    Ok(())
}

/// `trust_project_fetch_policy` accepts the same four spellings.
#[rstest]
#[case::true_spelling("true", true)]
#[case::one("1", true)]
#[case::false_spelling("false", false)]
#[case::zero("0", false)]
fn trust_project_fetch_policy_accepts_strict_spellings(
    #[case] raw: &str,
    #[case] expected: bool,
) -> Result<()> {
    let merged =
        merge_with_environment(&[("NETSUKE_TRUST_PROJECT_FETCH_POLICY", raw)], &["netsuke"])?;
    ensure!(
        merged.trust_project_fetch_policy == expected,
        "NETSUKE_TRUST_PROJECT_FETCH_POLICY={raw:?} should merge as {expected}, got {}",
        merged.trust_project_fetch_policy,
    );
    Ok(())
}

/// `no_input = 1` parses as Boolean `true` rather than a number or string.
#[test]
fn no_input_accepts_one_as_true() -> Result<()> {
    let merged = merge_with_environment(&[("NETSUKE_NO_INPUT", "1")], &["netsuke"])?;
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
    let error = merge_with_environment_error(&[("NETSUKE_NO_INPUT", "0")], &["netsuke"])?;
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
    let error = merge_with_file_and_environment_error("json = 1\n", &[], &["netsuke"])?;
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
        merge_with_file_and_environment("json = false\n", &[("NETSUKE_JSON", "1")], &["netsuke"])?;
    ensure!(
        merged.json,
        "NETSUKE_JSON=1 should override the file's json = false",
    );
    Ok(())
}
