//! JSON contracts for the typed Ninja launch and exit diagnostics.
//!
//! Both variants must expose the code, summary, help and cause chain the human
//! renderer shows, so an operator reading stderr and an automation reading the
//! document agree about which fault occurred and why.

use super::{first_diagnostic, render_diagnostic_json};
use crate::localization::{self, keys};
use crate::runner::{NinjaExitError, RunnerError};
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use serde_json::{Map, Value};
use std::process::ExitStatus;
use test_support::{EnLocalizer, en_localizer};

/// Report an exit status without spawning a process.
#[cfg(unix)]
fn exit_status(code: i32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;

    ExitStatus::from_raw(code << 8)
}

/// Report an exit status without spawning a process.
#[cfg(windows)]
fn exit_status(code: u32) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;

    ExitStatus::from_raw(code)
}

/// Build a launch failure for a Ninja that is not installed.
fn spawn_failure() -> RunnerError {
    RunnerError::NinjaSpawnFailed {
        program: String::from("ninja"),
        message: localization::message(keys::RUNNER_NINJA_SPAWN_FAILED)
            .with_arg("program", "ninja"),
        help: localization::message(keys::RUNNER_NINJA_SPAWN_FAILED_HELP)
            .with_arg("program", "ninja"),
        source: std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No such file or directory (os error 2)",
        ),
    }
}

/// Build an unsuccessful exit carrying command-list attribution.
fn exit_failure() -> RunnerError {
    let status = exit_status(7);
    RunnerError::NinjaFailed {
        status,
        operation: String::from("build"),
        message: localization::message(keys::RUNNER_NINJA_BUILD_FAILED).with_arg("status", status),
        source: NinjaExitError::new(status, "build", Some(String::from("action 0123, entry 2"))),
    }
}

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
    let document = render_diagnostic_json(&spawn_failure())?;
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
            == "Install Ninja and make sure `ninja` exists on PATH and is executable, or set \
                NETSUKE_NINJA to the path of a Ninja executable.",
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
    let document = render_diagnostic_json(&exit_failure())?;
    let value = serde_json::from_str::<Value>(&document)?;
    let diagnostic = first_diagnostic(&value)?;

    ensure!(
        string_field(diagnostic, "code")? == "netsuke::runner::ninja_failed",
        "the exit failure's code should be stable: {document}"
    );
    let message = without_isolation(string_field(diagnostic, "message")?);
    ensure!(
        message == "Ninja reported a build failure (exit status: 7).",
        "the summary should report Ninja's status: {message:?} in {document}"
    );
    ensure!(
        causes(diagnostic)? == ["ninja exited with exit status: 7: action 0123, entry 2"],
        "the cause should keep the legacy text and its attribution: {document}"
    );
    ensure!(
        !document.contains("with build file"),
        "the document must not name the temporary build file: {document}"
    );
    Ok(())
}
