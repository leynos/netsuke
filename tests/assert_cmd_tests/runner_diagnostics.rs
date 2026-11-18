//! Verify launch diagnostics, in human and JSON mode, through the real CLI.
//!
//! The launch diagnostic's whole promise is about what an operator sees: a
//! missing Ninja names the program and its remedies and does not blame the
//! temporary build file. `ninja_exit_diagnostics` covers the failed-recipe
//! promise and reuses the process helpers defined here.

use super::create_netsuke_command;
use anyhow::{Context, Result, ensure};
use netsuke::runner::NINJA_ENV;
use rstest::rstest;
use serde_json::Value;
use std::fs;
use std::path::Path;
use tempfile::{TempDir, tempdir};

/// Remove bidi isolates and terminal wrapping from localized output.
pub(super) fn flatten_localized_text(rendered: &str) -> String {
    let without_bidi_isolates = rendered
        .chars()
        .filter(|character| !matches!(*character, '\u{2068}' | '\u{2069}'))
        .collect::<String>();
    without_bidi_isolates
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Render the localized missing-manifest diagnostic, code, and help on stderr.
#[test]
fn missing_manifest_renders_runner_diagnostic_and_help() -> Result<()> {
    let temp = tempdir().context("create an empty workspace")?;
    let output = create_netsuke_command(temp.path())
        .env("NO_COLOR", "1")
        .args(["--locale", "en-GB", "generate"])
        .output()
        .context("run generate without a Netsukefile")?;
    ensure!(
        !output.status.success(),
        "generate must fail when the workspace has no Netsukefile"
    );
    let stderr = String::from_utf8(output.stderr).context("diagnostic stderr is UTF-8")?;
    let flattened = flatten_localized_text(&stderr);
    ensure!(
        flattened.contains("netsuke::runner::manifest_not_found"),
        "stderr must carry the runner's stable diagnostic code: {stderr}"
    );
    ensure!(
        flattened.contains("Manifest 'Netsukefile' not found in the current directory."),
        "stderr must carry the localized summary: {stderr}"
    );
    ensure!(
        flattened.contains("Ensure the manifest exists or pass `--file` with the correct path."),
        "stderr must carry the recovery help: {stderr}"
    );
    // The duplicate this guards against is the error text printed above the
    // rendering, so the count is the assertion rather than the presence of any
    // one line: both copies carry the summary, and a `contains` would pass
    // with either one removed.
    ensure!(
        flattened
            .matches("Manifest 'Netsukefile' not found")
            .count()
            == 1,
        "stderr must print the summary once, not above the diagnostic: {stderr}"
    );
    Ok(())
}

/// Stage the minimal manifest in a fresh workspace.
pub(super) fn workspace_with_minimal_manifest(context: &str) -> Result<TempDir> {
    let temp = tempdir().with_context(|| format!("create workspace for {context}"))?;
    fs::copy("tests/data/minimal.yml", temp.path().join("Netsukefile"))
        .with_context(|| format!("stage the minimal manifest for {context}"))?;
    Ok(temp)
}

/// Run `netsuke` with a child-only `PATH` and optional `NETSUKE_NINJA`.
///
/// Both values are set on the child alone. `PATH` decides whether a bare
/// `ninja` can be found, and `NETSUKE_NINJA` decides the program Netsuke
/// attempts, so each test states the exact launch it exercises instead of
/// inheriting whatever the host has installed.
pub(super) fn run_netsuke(
    workspace: &Path,
    path: &Path,
    ninja: Option<&Path>,
    arguments: &[&str],
) -> Result<std::process::Output> {
    let mut command = create_netsuke_command(workspace);
    command
        .env("NO_COLOR", "1")
        .env("PATH", path)
        .args(["--locale", "en-GB"])
        .args(arguments);
    if let Some(program) = ninja {
        command.env(NINJA_ENV, program);
    }
    command
        .output()
        .with_context(|| format!("run netsuke {arguments:?}"))
}

/// Assert the human launch diagnostic for `program`.
fn assert_spawn_failure_reported(stderr: &str, program: &str) -> Result<()> {
    let flattened = flatten_localized_text(stderr);
    ensure!(
        flattened.contains("netsuke::runner::ninja_spawn_failed"),
        "the launch failure must carry its stable code: {stderr}"
    );
    ensure!(
        flattened.contains(&format!("Could not start the Ninja program `{program}`.")),
        "the summary must name the attempted program: {stderr}"
    );
    ensure!(
        flattened.contains("Install Ninja and make sure"),
        "the help must offer the installation remedy: {stderr}"
    );
    ensure!(
        flattened.contains("NETSUKE_NINJA"),
        "the help must name the NETSUKE_NINJA override: {stderr}"
    );
    ensure!(
        !flattened.contains("with build file") && !flattened.contains(".ninja"),
        "the diagnostic must not name the temporary build file: {stderr}"
    );
    Ok(())
}

/// Read one string field from a JSON diagnostic object.
pub(super) fn string_field<'a>(diagnostic: &'a Value, name: &str) -> Result<&'a str> {
    diagnostic
        .get(name)
        .and_then(Value::as_str)
        .with_context(|| format!("the JSON diagnostic must carry a {name} string"))
}

/// Return the first diagnostic in a rendered JSON document.
pub(super) fn first_json_diagnostic(stderr: &str) -> Result<Value> {
    let document: Value = serde_json::from_str(stderr)
        .with_context(|| format!("stderr must be one JSON document: {stderr}"))?;
    document
        .get("diagnostics")
        .and_then(Value::as_array)
        .and_then(|entries| entries.first())
        .cloned()
        .context("the JSON document must carry the runner diagnostic")
}

/// Assert the JSON launch diagnostic for `program`.
fn assert_spawn_failure_json(stderr: &str, program: &str) -> Result<()> {
    let diagnostic = first_json_diagnostic(stderr)?;
    ensure!(
        string_field(&diagnostic, "code")? == "netsuke::runner::ninja_spawn_failed",
        "the JSON diagnostic must carry its stable code: {stderr}"
    );
    ensure!(
        flatten_localized_text(string_field(&diagnostic, "message")?)
            .contains(&format!("Could not start the Ninja program `{program}`.")),
        "the JSON summary must name the attempted program: {stderr}"
    );
    ensure!(
        flatten_localized_text(string_field(&diagnostic, "help")?).contains("NETSUKE_NINJA"),
        "the JSON help must name the NETSUKE_NINJA override: {stderr}"
    );
    let causes = diagnostic
        .get("causes")
        .and_then(Value::as_array)
        .context("the JSON diagnostic must carry its cause chain")?;
    ensure!(
        !causes.is_empty(),
        "the launch failure must keep its OS cause: {stderr}"
    );
    ensure!(
        !stderr.contains("with build file") && !stderr.contains(".ninja"),
        "the JSON diagnostic must not name the temporary build file: {stderr}"
    );
    Ok(())
}

/// Report a missing Ninja as a launch failure with its documented remedies.
#[test]
fn missing_ninja_reports_the_documented_summary() -> Result<()> {
    let temp = workspace_with_minimal_manifest("the documented missing-Ninja case")?;
    let empty_path = tempdir().context("create an empty PATH directory")?;
    let output = run_netsuke(
        temp.path(),
        empty_path.path(),
        None,
        &["--progress", "never", "build"],
    )?;
    ensure!(
        !output.status.success(),
        "a build without an installed Ninja must fail"
    );
    let stderr = String::from_utf8(output.stderr).context("diagnostic stderr is UTF-8")?;
    assert_spawn_failure_reported(&stderr, "ninja")
}

/// Report the launch failure in every rendering mode of both subcommands.
#[rstest]
#[case("build", "always", false)]
#[case("build", "always", true)]
#[case("build", "never", false)]
#[case("build", "never", true)]
#[case("clean", "always", false)]
#[case("clean", "always", true)]
#[case("clean", "never", false)]
#[case("clean", "never", true)]
fn missing_ninja_reports_the_documented_summary_across_modes(
    #[case] subcommand: &str,
    #[case] progress: &str,
    #[case] json: bool,
) -> Result<()> {
    let temp = workspace_with_minimal_manifest("a missing Ninja")?;
    let empty_path = tempdir().context("create an empty PATH directory")?;
    let mut arguments = vec!["--progress", progress];
    if json {
        arguments.push("--json");
    }
    arguments.push(subcommand);
    let output = run_netsuke(temp.path(), empty_path.path(), None, &arguments)?;
    ensure!(
        !output.status.success(),
        "{subcommand} must fail without an installed Ninja"
    );
    let stderr = String::from_utf8(output.stderr).context("diagnostic stderr is UTF-8")?;
    if json {
        assert_spawn_failure_json(&stderr, "ninja")
    } else {
        assert_spawn_failure_reported(&stderr, "ninja")
    }
}

/// Name the configured path when `NETSUKE_NINJA` points at a missing file.
#[test]
fn absent_ninja_override_names_the_configured_path() -> Result<()> {
    let temp = workspace_with_minimal_manifest("an absent Ninja override")?;
    let empty_path = tempdir().context("create an empty PATH directory")?;
    let missing = temp.path().join("tools").join("ninja");
    let output = run_netsuke(
        temp.path(),
        empty_path.path(),
        Some(&missing),
        &["--progress", "never", "build"],
    )?;
    ensure!(
        !output.status.success(),
        "a build naming an absent Ninja override must fail"
    );
    let stderr = String::from_utf8(output.stderr).context("diagnostic stderr is UTF-8")?;
    let program = missing.to_str().context("the configured path is UTF-8")?;
    assert_spawn_failure_reported(&stderr, program)
}

/// Report a Ninja program that exists but cannot be executed.
#[cfg(unix)]
#[test]
fn non_executable_ninja_is_reported_as_a_launch_failure() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let temp = workspace_with_minimal_manifest("a non-executable Ninja")?;
    let empty_path = tempdir().context("create an empty PATH directory")?;
    let script = temp.path().join("ninja-not-executable");
    fs::write(&script, "#!/bin/sh\nexit 0\n").context("write the non-executable Ninja file")?;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o644))
        .context("remove the execute bit from the Ninja file")?;

    let output = run_netsuke(
        temp.path(),
        empty_path.path(),
        Some(&script),
        &["--progress", "never", "build"],
    )?;
    ensure!(
        !output.status.success(),
        "a build with a non-executable Ninja must fail"
    );
    let stderr = String::from_utf8(output.stderr).context("diagnostic stderr is UTF-8")?;
    let program = script.to_str().context("the configured path is UTF-8")?;
    assert_spawn_failure_reported(&stderr, program)?;
    ensure!(
        flatten_localized_text(&stderr).contains("Permission denied"),
        "the launch failure must preserve the OS cause: {stderr}"
    );
    Ok(())
}
