//! Tests for the `env()` Jinja helper's variable resolution.
//!
//! These drive `env_var_with_default` directly, so nothing here mutates the
//! process environment and the cases run concurrently. The non-UTF-8 branch is
//! reachable only this way: fabricating such a value in the live environment
//! needs platform-specific `OsString` surgery, and the AGENTS.md testing
//! mandate forbids in-process mutation regardless.
//!
//! The `default` argument's *parsing* — which value kinds it accepts, and what
//! an absent or `none` argument means — belongs to the template layer and is
//! covered in `tests/manifest_env_tests.rs`. Here `default` has already been
//! reduced to an `Option<String>`, so the `none` and absent cases are one case.

use crate::manifest::{EnvAccessPolicy, EnvReadError, env_reader::env_var_with_default};
use minijinja::{Error, ErrorKind};
use rstest::rstest;

/// A reader outcome, as the `OBL-ENV-DEFAULT` partition enumerates it.
#[derive(Clone, Copy, Debug)]
enum Read {
    /// The variable is present with this value.
    Value(&'static str),
    /// The variable is absent.
    Absent,
    /// The variable is present but not valid UTF-8.
    NotUnicode,
}

/// The outcome a lookup under test must produce.
#[derive(Clone, Copy, Debug)]
enum Expected {
    /// The lookup succeeds with this value.
    Value(&'static str),
    /// The lookup fails as an undefined value.
    Missing,
    /// The lookup fails as an invalid operation.
    NotUnicode,
}

/// Resolve `Read::Value(_)` and `Read::Absent` through `fallback`.
fn resolve(read: Read, fallback: Option<&str>) -> Result<String, Error> {
    let read_result = match read {
        Read::Value(value) => Ok(value.to_owned()),
        Read::Absent => Err(EnvReadError::NotPresent),
        Read::NotUnicode => Err(EnvReadError::NotUnicode),
    };
    env_var_with_default(
        "FOO",
        &EnvAccessPolicy::default(),
        fallback.map(str::to_owned),
        |_| read_result.clone(),
    )
}

/// A resolution reduced to a comparable outcome.
///
/// `minijinja::Error` is not comparable, so a failure is reduced to its kind —
/// which is all the `OBL-ENV-DEFAULT` partition distinguishes, and all the
/// assertions below need.
#[derive(Debug, PartialEq)]
enum Resolved {
    /// The lookup produced this value.
    Value(String),
    /// The lookup failed with this kind.
    Failure(ErrorKind),
}

/// Assert that `read` under `fallback` produced `expected`.
fn assert_resolution(read: Read, fallback: Option<&str>, expected: Expected) {
    let observed = match resolve(read, fallback) {
        Ok(value) => Resolved::Value(value),
        Err(error) => Resolved::Failure(error.kind()),
    };
    let wanted = match expected {
        Expected::Value(want) => Resolved::Value(want.to_owned()),
        Expected::Missing => Resolved::Failure(ErrorKind::UndefinedError),
        Expected::NotUnicode => Resolved::Failure(ErrorKind::InvalidOperation),
    };
    assert_eq!(
        observed, wanted,
        "reader {read:?} with fallback {fallback:?}"
    );
}

/// `OBL-ENV-DEFAULT`: the fallback substitutes for absence, and nothing else.
///
/// The four reader outcomes are crossed with the fallback's presence. A present
/// value wins over the fallback even when it is the empty string — an empty
/// value is a value — and a non-UTF-8 value fails *even when* a fallback is
/// supplied, because a present-but-undecodable value is a configuration fault
/// rather than an absence.
#[rstest]
#[case::present_nonempty_without_fallback(Read::Value("value"), None, Expected::Value("value"))]
#[case::present_nonempty_with_fallback(
    Read::Value("value"),
    Some("fallback"),
    Expected::Value("value")
)]
#[case::present_empty_without_fallback(Read::Value(""), None, Expected::Value(""))]
#[case::present_empty_with_fallback(Read::Value(""), Some("fallback"), Expected::Value(""))]
#[case::absent_without_fallback(Read::Absent, None, Expected::Missing)]
#[case::absent_with_fallback(Read::Absent, Some("fallback"), Expected::Value("fallback"))]
#[case::non_utf8_without_fallback(Read::NotUnicode, None, Expected::NotUnicode)]
#[case::non_utf8_with_fallback(Read::NotUnicode, Some("fallback"), Expected::NotUnicode)]
fn default_substitutes_for_absence_only(
    #[case] read: Read,
    #[case] fallback: Option<&str>,
    #[case] expected: Expected,
) {
    assert_resolution(read, fallback, expected);
}

/// An empty fallback is a value, so it satisfies an absent variable.
#[test]
fn empty_fallback_satisfies_an_absent_variable() {
    assert_resolution(Read::Absent, Some(""), Expected::Value(""));
}

/// The two failures must be distinguishable.
///
/// A missing variable is a template authoring error, whereas a non-UTF-8 value
/// is an environment problem the author cannot fix in the template. Collapsing
/// them onto one kind would misdirect whoever reads the failure.
#[test]
fn the_two_failure_kinds_are_distinct() {
    let missing = resolve(Read::Absent, None).expect_err("missing");
    let non_utf8 = resolve(Read::NotUnicode, None).expect_err("non-UTF-8");
    assert_ne!(missing.kind(), non_utf8.kind());
}

/// The variable name reaches the seam unaltered but stays out of the message:
/// environment variable names routinely identify credentials, so the
/// diagnostic carries fixed text and the template location instead.
#[test]
fn the_requested_name_is_used_but_not_reported() {
    let mut observed = None;
    let err = env_var_with_default(
        "NETSUKE_SOME_VAR",
        &EnvAccessPolicy::default(),
        None,
        |key| {
            observed = Some(key.to_owned());
            Err(EnvReadError::NotPresent)
        },
    )
    .expect_err("should fail");
    assert_eq!(observed.as_deref(), Some("NETSUKE_SOME_VAR"));
    assert!(
        !err.to_string().contains("NETSUKE_SOME_VAR"),
        "the error must not name the variable, got {err}"
    );
}

/// A substituted fallback must not weaken the access policy.
///
/// The policy is evaluated first, so a blocked name never reaches the reader —
/// supplying a `default` must not turn the block into a successful read.
#[test]
fn a_fallback_does_not_bypass_the_access_policy() {
    let policy = EnvAccessPolicy::default().block_var("BLOCKED_VAR");
    let mut reader_was_called = false;
    let err = env_var_with_default(
        "BLOCKED_VAR",
        &policy,
        Some(String::from("fallback")),
        |_| {
            reader_was_called = true;
            Ok(String::from("value"))
        },
    )
    .expect_err("a blocked name must fail even with a default");
    assert!(
        !reader_was_called,
        "a blocked lookup must not call the reader"
    );
    assert_eq!(err.kind(), ErrorKind::InvalidOperation);
    assert!(
        !err.to_string().contains("BLOCKED_VAR"),
        "the error must not name the variable, got {err}"
    );
}
