//! Tests for the `env()` Jinja helper's variable resolution.
//!
//! These drive `env_var_with` directly, so nothing here mutates the process
//! environment and the cases run concurrently. The non-UTF-8 branch is
//! reachable only this way: fabricating such a value in the live environment
//! needs platform-specific `OsString` surgery, and the AGENTS.md testing
//! mandate forbids in-process mutation regardless.

use crate::manifest::{EnvReadError, env_var_with};
use minijinja::ErrorKind;
use rstest::rstest;

#[test]
fn present_variable_yields_its_value() {
    let value = env_var_with("FOO", |_| Ok(String::from("bar")));
    assert_eq!(value.expect("FOO should resolve"), "bar");
}

/// An empty value is a value, not an absence.
#[test]
fn empty_value_is_returned_rather_than_treated_as_missing() {
    let value = env_var_with("FOO", |_| Ok(String::new()));
    assert_eq!(value.expect("an empty value is still a value"), "");
}

#[rstest]
#[case::missing(EnvReadError::NotPresent, ErrorKind::UndefinedError)]
#[case::non_utf8(EnvReadError::NotUnicode, ErrorKind::InvalidOperation)]
fn failures_map_to_the_documented_jinja_error_kind(
    #[case] read_error: EnvReadError,
    #[case] expected: ErrorKind,
) {
    let err = env_var_with("FOO", |_| Err(read_error)).expect_err("should fail");
    assert_eq!(err.kind(), expected);
}

/// The two failures must be distinguishable.
///
/// A missing variable is a template authoring error, whereas a non-UTF-8 value
/// is an environment problem the author cannot fix in the template. Collapsing
/// them onto one kind would misdirect whoever reads the failure.
#[test]
fn the_two_failure_kinds_are_distinct() {
    let missing = env_var_with("FOO", |_| Err(EnvReadError::NotPresent)).expect_err("missing");
    let non_utf8 = env_var_with("FOO", |_| Err(EnvReadError::NotUnicode)).expect_err("non-UTF-8");
    assert_ne!(missing.kind(), non_utf8.kind());
}

/// The variable name reaches the seam unaltered but stays out of the message:
/// environment variable names routinely identify credentials, so the
/// diagnostic carries fixed text and the template location instead.
#[test]
fn the_requested_name_is_used_but_not_reported() {
    let mut observed = None;
    let err = env_var_with("NETSUKE_SOME_VAR", |key| {
        observed = Some(key.to_owned());
        Err(EnvReadError::NotPresent)
    })
    .expect_err("should fail");
    assert_eq!(observed.as_deref(), Some("NETSUKE_SOME_VAR"));
    assert!(
        !err.to_string().contains("NETSUKE_SOME_VAR"),
        "the error must not name the variable, got {err}"
    );
}
