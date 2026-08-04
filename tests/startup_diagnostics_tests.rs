//! End-to-end tests for startup locale diagnostics.
//!
//! These run the built binary rather than calling into the library, because the
//! behaviour under test is a property of the whole startup sequence: the
//! subscriber is installed, the locale is resolved before the command line is
//! parsed, events are buffered, and the buffer is settled on whichever path the
//! run takes — including the ones that terminate inside `clap` and never return
//! to `run_with_args`.
//!
//! A unit test cannot cover that. An earlier one called `build_localizer`
//! directly and passed while the buffered warning was being dropped on every
//! early-exit path, because `clap::Error::exit` never returns and the settle
//! call sat after it.

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use tempfile::TempDir;
use test_support::netsuke::{NetsukeRun, run_netsuke_in_with_env};

/// A locale that ships no catalogue, and whose language ships none either, so
/// it resolves to the English source and reports a fallback.
const UNSUPPORTED_LOCALE: &str = "is-IS";

/// The message `build_localizer` emits when a request cannot be honoured.
const FALLBACK_WARNING: &str = "falling back to the source locale";

/// Run the binary with an explicitly empty environment, in an empty directory.
///
/// `run_netsuke_in_with_env` clears the child's environment rather than
/// inheriting it, so what these tests observe depends only on the arguments
/// they pass. That matters here more than most: the behaviour under test is
/// how a *locale* is resolved, and `NETSUKE_LOCALE` in the developer's or CI
/// environment would otherwise silently take part. The temporary directory
/// does the same for the working tree, so `build` cannot find the
/// repository's own manifest.
fn run(args: &[&str]) -> Result<NetsukeRun> {
    let directory = TempDir::new().context("stage an empty working directory")?;
    run_netsuke_in_with_env(directory.path(), args, &[])
}

fn stderr_of(run: &NetsukeRun) -> String {
    run.stderr.clone()
}

/// Human mode must report an unsupported startup locale, whichever way the run
/// ends.
///
/// The three cases are the paths that leave `run_with_args` differently: a
/// usage error and `--help` both terminate inside `clap`, while a real command
/// runs on to the configuration merge.
#[rstest]
// Terminates inside clap, after printing a usage error.
#[case(&["--locale", UNSUPPORTED_LOCALE, "--not-a-flag"])]
// Terminates inside clap, after printing help.
#[case(&["--locale", UNSUPPORTED_LOCALE, "--help"])]
// Runs on past the configuration merge.
#[case(&["--locale", UNSUPPORTED_LOCALE, "build"])]
fn human_mode_reports_an_unsupported_startup_locale(#[case] args: &[&str]) -> Result<()> {
    let output = run(args)?;
    let stderr = stderr_of(&output);
    ensure!(
        stderr.contains(FALLBACK_WARNING),
        "expected the fallback warning on stderr for {args:?}, got: {stderr}"
    );
    ensure!(
        stderr.contains(UNSUPPORTED_LOCALE),
        "the warning must name the requested locale, got: {stderr}"
    );
    Ok(())
}

/// A supported locale must not warn, or the warning would appear on every run
/// and stop carrying information.
#[test]
fn human_mode_stays_quiet_for_a_supported_locale() -> Result<()> {
    let output = run(&["--locale", "fr", "--not-a-flag"])?;
    let stderr = stderr_of(&output);
    ensure!(
        !stderr.contains(FALLBACK_WARNING),
        "a shipped catalogue must not warn, got: {stderr}"
    );
    Ok(())
}

/// JSON mode must leave stderr carrying exactly one diagnostic document.
///
/// This is the requirement the buffering exists for: the fallback happens
/// before the effective mode is known, and the diagnostic is written to stderr,
/// so an eagerly emitted warning would corrupt the document a consumer parses.
#[test]
fn json_mode_emits_only_the_diagnostic_document() -> Result<()> {
    let output = run(&["--locale", UNSUPPORTED_LOCALE, "--not-a-flag", "--json"])?;
    let stderr = stderr_of(&output);

    ensure!(
        !stderr.contains(FALLBACK_WARNING),
        "JSON mode must not write the fallback warning to stderr, got: {stderr}"
    );
    // Parsing the whole stream is the assertion: anything emitted beside the
    // document — before or after — makes it fail.
    let parsed: serde_json::Value = serde_json::from_str(&stderr)
        .with_context(|| format!("stderr must be exactly one JSON document, got: {stderr}"))?;
    ensure!(
        parsed.get("schema_version").is_some(),
        "expected a diagnostic document, got: {parsed}"
    );
    Ok(())
}

/// The JSON help path writes nothing to stderr at all.
#[test]
fn json_mode_help_leaves_stderr_empty() -> Result<()> {
    let output = run(&["--locale", UNSUPPORTED_LOCALE, "--help", "--json"])?;
    let stderr = stderr_of(&output);
    ensure!(
        stderr.is_empty(),
        "expected empty stderr on the JSON help path, got: {stderr}"
    );
    Ok(())
}
