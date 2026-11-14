//! Bounded-diagnostic tests for rejected shell definitions.
//!
//! A rejection diagnostic must name the offending field while never echoing an
//! argument vector, an offending value in full, or an ambient environment
//! value: those can carry operator secrets, and the message reaches both a
//! terminal and a machine consumer.

use super::{
    OPERATOR_CONFIG, RELATIVE_EXECUTABLE, SENTINEL, Startup, definition_body,
    ensure_rejected_before_compilation, generate_with_operator_config, stderr_of,
};
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

/// Name the file that declared a rejected definition.
///
/// The same key and index can appear in several layers, so a diagnostic that
/// named only `shells[0].executable` would leave the operator guessing which
/// file to edit.
#[test]
fn a_rejection_names_the_declaring_file() -> Result<()> {
    let startup = Startup::new()?;
    let output = generate_with_operator_config(&startup, RELATIVE_EXECUTABLE)?;

    ensure_rejected_before_compilation(&startup, &output, "shells[0].executable")?;
    let stderr = stderr_of(&output);
    let declaring = startup.path().join(OPERATOR_CONFIG);
    ensure!(
        stderr.contains(&declaring.to_string_lossy().into_owned()),
        "diagnostic should name {}, got: {stderr}",
        declaring.display()
    );
    Ok(())
}

/// Bound and escape the declaring path before it reaches the terminal.
///
/// A directory name is operator-supplied text, so it may contain a control
/// character or be far longer than a readable diagnostic should quote. The
/// preview must escape the former and truncate the latter, exactly as a
/// rejected value is rendered.
#[rstest]
#[case::control_character("odd\u{7}directory", "\\u{7}", "\u{7}")]
#[case::overlong_name(&"x".repeat(90), "…", "")]
fn the_declaring_path_is_bounded_and_escaped(
    #[case] directory: &str,
    #[case] expected: &str,
    #[case] forbidden: &str,
) -> Result<()> {
    let startup = Startup::new()?;
    let untrusted = startup.write(
        &format!("{directory}/untrusted.toml"),
        &definition_body("dash"),
    )?;
    let selector = untrusted.to_string_lossy().into_owned();

    let output = startup
        .command(&["--config", &selector, "generate", "--output", SENTINEL])
        .output()
        .context("run netsuke with an explicit selector")?;

    ensure_rejected_before_compilation(&startup, &output, "shells")?;
    let stderr = stderr_of(&output);
    ensure!(
        stderr.contains(expected),
        "diagnostic should render {expected:?}, got: {stderr}"
    );
    if !forbidden.is_empty() {
        ensure!(
            !stderr.contains(forbidden),
            "diagnostic should not emit the raw {forbidden:?}, got: {stderr}"
        );
    }
    Ok(())
}
