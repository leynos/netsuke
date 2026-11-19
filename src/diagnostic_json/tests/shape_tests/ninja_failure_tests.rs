//! JSON contracts for the typed Ninja launch and exit diagnostics.
//!
//! Both variants must expose the code, summary, help and cause chain the human
//! renderer shows, so an operator reading stderr and an automation reading the
//! document agree about which fault occurred and why.

use super::{first_diagnostic, render_diagnostic_json};
use crate::runner::test_fixtures::{ninja_exit_failure, ninja_spawn_failure};
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use serde_json::{Map, Value};
use test_support::{EnLocalizer, en_localizer, exit_status};

/// Read one string field from a diagnostic object.
fn string_field<'a>(diagnostic: &'a Map<String, Value>, name: &str) -> Result<&'a str> {
    diagnostic
        .get(name)
        .and_then(Value::as_str)
        .with_context(|| format!("the diagnostic should carry a {name} string"))
}

/// Remove the bidi isolation controls Fluent wraps around each placeable.
///
/// The interpolated values stay isolated in the published document, which is
/// what keeps them from reordering the surrounding sentence; the tests compare
/// the words, so they strip the controls rather than reproduce them.
fn without_isolation(text: &str) -> String {
    text.chars()
        .filter(|character| {
            !matches!(
                character,
                '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{2066}'..='\u{2069}'
            )
        })
        .collect()
}

/// Read the cause list from a diagnostic object.
fn causes(diagnostic: &Map<String, Value>) -> Result<Vec<&str>> {
    diagnostic
        .get("causes")
        .and_then(Value::as_array)
        .context("the diagnostic should carry a causes array")?
        .iter()
        .map(|cause| cause.as_str().context("each cause should be a string"))
        .collect()
}

/// A launch failure publishes its code, program, help, and OS cause.
///
/// Reporting the invocation context instead would leave `code` and `help` null
/// and make the only cause the temporary build file, which is the defect this
/// diagnostic replaces.
#[rstest]
fn spawn_failure_json_names_program_and_remedies(en_localizer: EnLocalizer) -> Result<()> {
    let _en_localizer = en_localizer;
    let document = render_diagnostic_json(&ninja_spawn_failure()?)?;
    let value = serde_json::from_str::<Value>(&document)?;
    let diagnostic = first_diagnostic(&value)?;

    ensure!(
        string_field(diagnostic, "code")? == "netsuke::runner::ninja_spawn_failed",
        "the launch failure's code should be stable: {document}"
    );
    ensure!(
        without_isolation(string_field(diagnostic, "message")?)
            == "Could not start the Ninja program `ninja`.",
        "the summary should name the attempted program: {document}"
    );
    ensure!(
        without_isolation(string_field(diagnostic, "help")?)
            == "Install Ninja and make sure `ninja` can be executed, or set NETSUKE_NINJA to \
                the path of a Ninja executable.",
        "the help should offer the installation and configuration remedies: {document}"
    );
    ensure!(
        causes(diagnostic)? == ["No such file or directory (os error 2)"],
        "the OS cause should survive into the document: {document}"
    );
    ensure!(
        diagnostic.get("source").is_some_and(Value::is_null),
        "a launch failure should not claim a source file span: {document}"
    );
    ensure!(
        !document.contains("with build file"),
        "the document must not name the temporary build file: {document}"
    );
    Ok(())
}

/// An unsuccessful exit publishes its status and keeps the attribution cause.
#[rstest]
fn exit_failure_json_reports_status_and_attribution(en_localizer: EnLocalizer) -> Result<()> {
    let _en_localizer = en_localizer;
    let document = render_diagnostic_json(&ninja_exit_failure(exit_status(7))?)?;
    let value = serde_json::from_str::<Value>(&document)?;
    let diagnostic = first_diagnostic(&value)?;

    ensure!(
        string_field(diagnostic, "code")? == "netsuke::runner::ninja_failed",
        "the exit failure's code should be stable: {document}"
    );
    let message = without_isolation(string_field(diagnostic, "message")?);
    // The summary is localized, so it does not carry the platform-defined
    // `ExitStatus` wording; the cause chain still does.
    ensure!(
        message == "Ninja reported a build failure (exit status 7).",
        "the summary should report Ninja's status: {message:?} in {document}"
    );
    let status = exit_status(7);
    let expected_cause = format!("ninja exited with {status}: action 0123, entry 2");
    ensure!(
        causes(diagnostic)? == [expected_cause.as_str()],
        "the cause should keep the legacy text and its attribution: {document}"
    );
    ensure!(
        !document.contains("with build file"),
        "the document must not name the temporary build file: {document}"
    );
    Ok(())
}

/// A signal-terminated Ninja publishes the signal suffix, not an exit status.
///
/// `ExitStatus::code` is `None` for a signal, so formatting the status the way
/// an ordinary exit is formatted would either invent a status or fall back to
/// the platform's English wording. The summary must select the catalogue's
/// signal suffix instead, and must not claim a numeric exit status.
#[cfg(unix)]
#[rstest]
fn signal_terminated_exit_json_reports_the_signal_suffix(en_localizer: EnLocalizer) -> Result<()> {
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    let _en_localizer = en_localizer;
    let document = render_diagnostic_json(&ninja_exit_failure(ExitStatus::from_raw(15))?)?;
    let value = serde_json::from_str::<Value>(&document)?;
    let diagnostic = first_diagnostic(&value)?;

    ensure!(
        string_field(diagnostic, "code")? == "netsuke::runner::ninja_failed",
        "the signal failure's code should stay stable: {document}"
    );
    let message = without_isolation(string_field(diagnostic, "message")?);
    ensure!(
        message == "Ninja reported a build failure (terminated by signal).",
        "the summary should report the terminating signal: {message:?} in {document}"
    );
    ensure!(
        !message.contains("exit status"),
        "a signal-terminated summary must not claim an exit status: {message:?}"
    );
    Ok(())
}
