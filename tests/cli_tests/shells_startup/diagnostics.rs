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
use std::path::Path;

/// Mark a value that is visible only to the child process's environment.
const AMBIENT_MARKER: &str = "ambient-marker-value-that-must-not-be-printed";

/// Text a diagnostic prints immediately before quoting the declaring file.
const DECLARED_IN: &str = "declared in \"";

/// Mirror the quoting budget of `src/cli/config/shells/preview.rs`.
///
/// A declaring path longer than this is quoted only as a prefix, so the
/// directory holding the file may legitimately fall outside the preview.
const DIAGNOSTIC_BUDGET: usize = 64;

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
/// file to edit. The diagnostic renders the declaring path through the same
/// escaping and length bound as any other rejected text, and discovery reports
/// the canonical spelling of a path rather than the one the harness happened to
/// create, so the assertion compares against both: the quoted preview must be a
/// faithful escaped prefix of the canonical path, and, when the directory
/// holding the file lies inside the quoting budget, it must also name that
/// directory.
#[test]
fn a_rejection_names_the_declaring_file() -> Result<()> {
    let startup = Startup::new()?;
    let output = generate_with_operator_config(&startup, RELATIVE_EXECUTABLE)?;

    ensure_rejected_before_compilation(&startup, &output, "shells[0].executable")?;
    let stderr = stderr_of(&output);
    let declaring = startup.path().join(OPERATOR_CONFIG);
    let canonical = dunce::canonicalize(&declaring)
        .with_context(|| format!("canonicalize {}", declaring.display()))?;
    let escaped = escaped_text(&canonical.to_string_lossy());
    let shown = declared_preview(&stderr)?;
    let prefix = shown.strip_suffix('…').unwrap_or(shown);

    ensure!(
        !prefix.is_empty() && escaped.starts_with(prefix),
        "diagnostic should quote a prefix of {escaped}, got: {shown:?}"
    );
    let directory = Path::new(OPERATOR_CONFIG)
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .context("the operator configuration should sit in a named directory")?;
    // The preview quotes only the first `DIAGNOSTIC_BUDGET` characters of the
    // path, so a long temporary root legitimately hides the directory name.
    let within_budget = canonical
        .to_string_lossy()
        .chars()
        .take(DIAGNOSTIC_BUDGET)
        .collect::<String>()
        .contains(directory);
    ensure!(
        !within_budget || prefix.contains(directory),
        "diagnostic should reach the configuration directory {directory:?}, got: {shown:?}"
    );
    Ok(())
}

/// Bound an overlong declaring path before it reaches the terminal.
///
/// A directory name is operator-supplied text, so it may be far longer than a
/// readable diagnostic should quote. The preview must truncate it and say so.
/// Escaping has its own tests beside the renderer, because a control character
/// cannot appear in a directory name on every platform.
#[test]
fn an_overlong_declaring_path_is_bounded() -> Result<()> {
    let startup = Startup::new()?;
    let untrusted = startup.write(
        &format!("{}/untrusted.toml", "x".repeat(90)),
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
        declared_preview(&stderr)?.ends_with('…'),
        "diagnostic should mark a truncated path, got: {stderr}"
    );
    Ok(())
}

/// Read the quoted preview the diagnostic printed for the declaring file.
fn declared_preview(stderr: &str) -> Result<&str> {
    let (_, quoted) = stderr
        .split_once(DECLARED_IN)
        .with_context(|| format!("diagnostic should name a declaring file, got: {stderr}"))?;
    quoted
        .split_once('"')
        .map(|(preview, _)| preview)
        .with_context(|| format!("diagnostic should close the quoted preview, got: {quoted}"))
}

/// Render text the way a diagnostic does before quoting it.
///
/// Control characters take their `escape_debug` spelling. Unlike the production
/// renderer this never truncates, because the caller allows for the bound
/// separately.
fn escaped_text(text: &str) -> String {
    text.chars().flat_map(char::escape_debug).collect()
}
