//! Verify the failed-recipe diagnostic through the real CLI process.
//!
//! The exit diagnostic's whole promise is about what an operator sees: it
//! states that Ninja reported a failure, carries Ninja's status, and does not
//! blame the temporary build file. The launch diagnostic lives beside this
//! module in `runner_diagnostics`, which owns the shared process helpers.

use super::runner_diagnostics::{
    first_json_diagnostic, flatten_localized_text, run_netsuke, string_field,
    workspace_with_minimal_manifest,
};
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use serde_json::Value;
use tempfile::tempdir;
use test_support::fake_ninja;

/// Format the exit status as the child process's own `Display` renders it.
///
/// Only the cause chain keeps Ninja's legacy text, whose wording is platform
/// defined; the localized summary supplies its own wording.
fn exit_cause_status_text() -> &'static str {
    if cfg!(windows) {
        "exit code: 7"
    } else {
        "exit status: 7"
    }
}

/// Status phrase the English summary's localized suffix supplies.
const SUMMARY_STATUS: &str = "exit status 7";

/// Assert the human exit diagnostic for `subcommand`.
fn assert_exit_failure_reported(stderr: &str, subcommand: &str) -> Result<()> {
    let flattened = flatten_localized_text(stderr);
    ensure!(
        flattened.contains("netsuke::runner::ninja_failed"),
        "the exit failure must carry its stable code: {stderr}"
    );
    let summary = if subcommand == "build" {
        format!("Ninja reported a build failure ({SUMMARY_STATUS}).")
    } else {
        format!("Ninja tool `{subcommand}` reported a failure ({SUMMARY_STATUS}).")
    };
    ensure!(
        flattened.contains(&summary),
        "the summary must report Ninja's status and operation: {stderr}"
    );
    let cause_status = exit_cause_status_text();
    ensure!(
        flattened.contains(&format!("ninja exited with {cause_status}")),
        "the cause must keep Ninja's legacy exit text: {stderr}"
    );
    ensure!(
        !flattened.contains("with build file") && !flattened.contains("netsuke."),
        "the diagnostic must not name the temporary build file: {stderr}"
    );
    Ok(())
}

/// Assert the JSON exit diagnostic for `subcommand`.
fn assert_exit_failure_json(stderr: &str, subcommand: &str) -> Result<()> {
    let diagnostic = first_json_diagnostic(stderr)?;
    ensure!(
        string_field(&diagnostic, "code")? == "netsuke::runner::ninja_failed",
        "the JSON diagnostic must carry its stable code: {stderr}"
    );
    let summary = if subcommand == "build" {
        format!("Ninja reported a build failure ({SUMMARY_STATUS}).")
    } else {
        format!("Ninja tool `{subcommand}` reported a failure ({SUMMARY_STATUS}).")
    };
    let message = flatten_localized_text(string_field(&diagnostic, "message")?);
    ensure!(
        message == summary,
        "the JSON summary must report Ninja's status, got {message:?} in {stderr}"
    );
    let cause_status = exit_cause_status_text();
    let causes = diagnostic
        .get("causes")
        .and_then(Value::as_array)
        .context("the JSON diagnostic must carry its cause chain")?;
    ensure!(
        causes
            .iter()
            .filter_map(Value::as_str)
            .any(|cause| cause.contains(&format!("ninja exited with {cause_status}"))),
        "the JSON cause must keep Ninja's exit text: {stderr}"
    );
    ensure!(
        !stderr.contains("with build file") && !stderr.contains("netsuke."),
        "the JSON diagnostic must not name the temporary build file: {stderr}"
    );
    Ok(())
}

/// Report an unsuccessful recipe with Ninja's own exit status.
#[test]
fn recipe_failure_reports_ninja_status() -> Result<()> {
    let temp = workspace_with_minimal_manifest("a failing recipe")?;
    let empty_path = tempdir().context("create an empty PATH directory")?;
    let (_ninja_dir, ninja) = fake_ninja(7)?;
    let output = run_netsuke(
        temp.path(),
        empty_path.path(),
        Some(&ninja),
        &["--progress", "never", "build"],
    )?;
    ensure!(
        !output.status.success(),
        "a failing recipe must fail the build"
    );
    let stderr = String::from_utf8(output.stderr).context("diagnostic stderr is UTF-8")?;
    assert_exit_failure_reported(&stderr, "build")
}

/// Report Ninja's exit status in every rendering mode of both subcommands.
#[rstest]
#[case("build", "always", false)]
#[case("build", "always", true)]
#[case("build", "never", false)]
#[case("build", "never", true)]
#[case("clean", "always", false)]
#[case("clean", "always", true)]
#[case("clean", "never", false)]
#[case("clean", "never", true)]
fn recipe_failure_reports_ninja_status_across_modes(
    #[case] subcommand: &str,
    #[case] progress: &str,
    #[case] json: bool,
) -> Result<()> {
    let temp = workspace_with_minimal_manifest("a failing recipe")?;
    let empty_path = tempdir().context("create an empty PATH directory")?;
    let (_ninja_dir, ninja) = fake_ninja(7)?;
    let mut arguments = vec!["--progress", progress];
    if json {
        arguments.push("--json");
    }
    arguments.push(subcommand);
    let output = run_netsuke(temp.path(), empty_path.path(), Some(&ninja), &arguments)?;
    ensure!(
        !output.status.success(),
        "{subcommand} must fail when Ninja reports an unsuccessful exit"
    );
    if json {
        ensure!(
            output.stdout.is_empty(),
            "a failing JSON run must not write to stdout: {:?}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
    let stderr = String::from_utf8(output.stderr).context("diagnostic stderr is UTF-8")?;
    if json {
        assert_exit_failure_json(&stderr, subcommand)
    } else {
        assert_exit_failure_reported(&stderr, subcommand)
    }
}
