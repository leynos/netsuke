//! Bounded-diagnostic tests for rejected shell definitions.
//!
//! A rejection diagnostic must name the offending field while never echoing an
//! argument vector, an offending value in full, or an ambient environment
//! value: those can carry operator secrets, and the message reaches both a
//! terminal and a machine consumer.

use super::{SENTINEL, Startup, ensure_rejected_before_compilation, stderr_of};
use anyhow::{Context, Result, ensure};
use rstest::rstest;

/// Mark a value that is visible only to the child process's environment.
const AMBIENT_MARKER: &str = "ambient-marker-value-that-must-not-be-printed";

#[rstest]
#[case::non_string_argument(
    "[[shells]]\nname = \"dash\"\nexecutable = \"dash\"\nargs = [\"super-secret-token\", 42]\n",
    "shells[0].args",
    "argument 1 must be a string",
    "super-secret-token"
)]
#[case::oversized_name(
    "[[shells]]\nname = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"\nexecutable = \"dash\"\nargs = [\"-c\"]\n",
    "shells[0].name",
    "at most 63 bytes",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
)]
#[case::control_character_name(
    "[[shells]]\nname = \"dash\\u0007\"\nexecutable = \"dash\"\nargs = [\"-c\"]\n",
    "shells[0].name",
    "invalid shell name character",
    "\u{7}"
)]
fn diagnostics_are_bounded_and_never_echo_argument_vectors(
    #[case] definition: &str,
    #[case] key: &str,
    #[case] expected: &str,
    #[case] forbidden: &str,
) -> Result<()> {
    let startup = Startup::new()?;
    startup.write("user/netsuke/config.toml", definition)?;

    // An ambient value must never be echoed either. It is set only for the
    // child, so the assertion cannot be satisfied by the parent's environment.
    let output = startup
        .command(&["generate", "--output", SENTINEL])
        .env("AMBIENT_TEST_MARKER", AMBIENT_MARKER)
        .output()
        .context("run netsuke with an ambient marker")?;

    ensure_rejected_before_compilation(&startup, &output, key)?;
    let stderr = stderr_of(&output);
    ensure!(
        stderr.contains(expected),
        "diagnostic should contain {expected:?}, got: {stderr}"
    );
    ensure!(
        !stderr.contains(forbidden),
        "diagnostic should not echo {forbidden:?}, got: {stderr}"
    );
    ensure!(
        !stderr.contains(AMBIENT_MARKER),
        "diagnostic should not echo an ambient value, got: {stderr}"
    );
    Ok(())
}
