//! Verify the launch diagnostic for a missing or non-executable Ninja.
//!
//! The launch diagnostic's whole promise is about what an operator sees: the
//! summary names the program Netsuke attempted, the help offers the remedies,
//! and neither blames the temporary build file. These cases run the real
//! command line interface in human and JSON output, across both subcommands
//! and both progress policies, so every rendering path meets the same
//! assertion.

use super::generated_build_file::names_generated_build_file;
use super::runner_diagnostics::{
    absent_program_cause, first_json_diagnostic, flatten_localized_text, missing_ninja_override,
    missing_ninja_program, run_netsuke, string_field, workspace_with_minimal_manifest,
};
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use serde_json::Value;
use std::fs;
use tempfile::tempdir;

/// One command-line invocation a rendering matrix case exercises.
///
/// The cases differ only in how Netsuke is invoked, so each row states one
/// value and the argument list the run needs is derived from it in one place.
#[derive(Clone, Copy)]
struct Invocation {
    /// Subcommand under test: `build` or `clean`.
    subcommand: &'static str,
    /// Console policy under test: `always` or `never`.
    progress: &'static str,
    /// Whether the case reads the JSON renderer.
    json: bool,
}

impl Invocation {
    /// Return the arguments selecting this invocation on the command line.
    fn arguments(self) -> Vec<&'static str> {
        let mut arguments = vec!["--progress", self.progress];
        if self.json {
            arguments.push("--json");
        }
        arguments.push(self.subcommand);
        arguments
    }
}

/// The launch diagnostic an operator should see for one attempted program.
///
/// The launch fixtures differ only in the program Netsuke attempts and the
/// operating-system cause the platform reports, so they hand the assertions
/// one value describing the launch rather than two loose strings.
struct LaunchDiagnostic<'a> {
    /// The program Netsuke was asked to start.
    program: &'a str,
    /// The text the platform reports for this particular launch failure.
    cause: &'a str,
}

/// Assert the human launch diagnostic for the attempted program.
fn assert_spawn_failure_reported(stderr: &str, launch: &LaunchDiagnostic<'_>) -> Result<()> {
    let flattened = flatten_localized_text(stderr);
    ensure!(
        flattened.contains("netsuke::runner::ninja_spawn_failed"),
        "the launch failure must carry its stable code: {stderr}"
    );
    let expected = flatten_localized_text(&format!(
        "Could not start the Ninja program `{}`.",
        launch.program
    ));
    ensure!(
        flattened.contains(&expected),
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
        !flattened.contains("with build file") && !names_generated_build_file(&flattened),
        "the diagnostic must not name the temporary build file: {stderr}"
    );
    Ok(())
}

/// Assert the JSON launch diagnostic for the attempted program.
fn assert_spawn_failure_json(stderr: &str, launch: &LaunchDiagnostic<'_>) -> Result<()> {
    let diagnostic = first_json_diagnostic(stderr)?;
    ensure!(
        string_field(&diagnostic, "code")? == "netsuke::runner::ninja_spawn_failed",
        "the JSON diagnostic must carry its stable code: {stderr}"
    );
    let expected = flatten_localized_text(&format!(
        "Could not start the Ninja program `{}`.",
        launch.program
    ));
    ensure!(
        flatten_localized_text(string_field(&diagnostic, "message")?).contains(&expected),
        "the JSON summary must name the attempted program: {stderr}"
    );
    let help = flatten_localized_text(string_field(&diagnostic, "help")?);
    ensure!(
        help.contains("Install Ninja and make sure") && help.contains("NETSUKE_NINJA"),
        "the JSON help must name both remedies: {stderr}"
    );
    let causes = diagnostic
        .get("causes")
        .and_then(Value::as_array)
        .context("the JSON diagnostic must carry its cause chain")?;
    // A non-empty array of nulls would satisfy a length check while carrying
    // no cause at all, so each fixture states the text the platform reports.
    ensure!(
        causes
            .iter()
            .filter_map(Value::as_str)
            .any(|cause| cause.contains(launch.cause)),
        "the launch failure must keep its operating-system cause: {stderr}"
    );
    ensure!(
        !stderr.contains("with build file") && !names_generated_build_file(stderr),
        "the JSON diagnostic must not name the temporary build file: {stderr}"
    );
    Ok(())
}

/// Report a missing Ninja as a launch failure with its documented remedies.
#[test]
fn missing_ninja_reports_the_documented_summary() -> Result<()> {
    let temp = workspace_with_minimal_manifest("the documented missing-Ninja case")?;
    let empty_path = tempdir().context("create an empty PATH directory")?;
    let launch = LaunchDiagnostic {
        program: missing_ninja_program(),
        cause: absent_program_cause(),
    };
    let output = run_netsuke(
        temp.path(),
        empty_path.path(),
        missing_ninja_override(),
        &["--progress", "never", "build"],
    )?;
    ensure!(
        !output.status.success(),
        "a build without an installed Ninja must fail"
    );
    let stderr = String::from_utf8(output.stderr).context("diagnostic stderr is UTF-8")?;
    assert_spawn_failure_reported(&stderr, &launch)
}

/// Report the launch failure in every rendering mode of both subcommands.
///
/// The human `build --progress never` case is owned by
/// [`missing_ninja_reports_the_documented_summary`], which the issue names, so
/// this matrix does not repeat it.
#[rstest]
#[case(Invocation { subcommand: "build", progress: "always", json: false })]
#[case(Invocation { subcommand: "build", progress: "always", json: true })]
#[case(Invocation { subcommand: "build", progress: "never", json: true })]
#[case(Invocation { subcommand: "clean", progress: "always", json: false })]
#[case(Invocation { subcommand: "clean", progress: "always", json: true })]
#[case(Invocation { subcommand: "clean", progress: "never", json: false })]
#[case(Invocation { subcommand: "clean", progress: "never", json: true })]
fn missing_ninja_reports_the_documented_summary_across_modes(
    #[case] invocation: Invocation,
) -> Result<()> {
    let temp = workspace_with_minimal_manifest("a missing Ninja")?;
    let empty_path = tempdir().context("create an empty PATH directory")?;
    let output = run_netsuke(
        temp.path(),
        empty_path.path(),
        missing_ninja_override(),
        &invocation.arguments(),
    )?;
    ensure!(
        !output.status.success(),
        "{} must fail without an installed Ninja",
        invocation.subcommand
    );
    if invocation.json {
        ensure!(
            output.stdout.is_empty(),
            "a failing JSON run must not write to stdout: {:?}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
    let stderr = String::from_utf8(output.stderr).context("diagnostic stderr is UTF-8")?;
    let launch = LaunchDiagnostic {
        program: missing_ninja_program(),
        cause: absent_program_cause(),
    };
    if invocation.json {
        assert_spawn_failure_json(&stderr, &launch)
    } else {
        assert_spawn_failure_reported(&stderr, &launch)
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
    let launch = LaunchDiagnostic {
        program: missing.to_str().context("the configured path is UTF-8")?,
        cause: absent_program_cause(),
    };
    assert_spawn_failure_reported(&stderr, &launch)
}

/// Report a Ninja program that exists but cannot be executed, in every mode.
#[cfg(unix)]
#[rstest]
#[case(Invocation { subcommand: "build", progress: "always", json: false })]
#[case(Invocation { subcommand: "build", progress: "always", json: true })]
#[case(Invocation { subcommand: "build", progress: "never", json: false })]
#[case(Invocation { subcommand: "build", progress: "never", json: true })]
#[case(Invocation { subcommand: "clean", progress: "always", json: false })]
#[case(Invocation { subcommand: "clean", progress: "always", json: true })]
#[case(Invocation { subcommand: "clean", progress: "never", json: false })]
#[case(Invocation { subcommand: "clean", progress: "never", json: true })]
fn non_executable_ninja_is_reported_as_a_launch_failure(
    #[case] invocation: Invocation,
) -> Result<()> {
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
        &invocation.arguments(),
    )?;
    ensure!(
        !output.status.success(),
        "{} with a non-executable Ninja must fail",
        invocation.subcommand
    );
    if invocation.json {
        ensure!(
            output.stdout.is_empty(),
            "a failing JSON run must not write to stdout: {:?}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
    let stderr = String::from_utf8(output.stderr).context("diagnostic stderr is UTF-8")?;
    let launch = LaunchDiagnostic {
        program: script.to_str().context("the configured path is UTF-8")?,
        cause: "Permission denied",
    };
    if invocation.json {
        assert_spawn_failure_json(&stderr, &launch)
    } else {
        assert_spawn_failure_reported(&stderr, &launch)?;
        ensure!(
            flatten_localized_text(&stderr).contains("Permission denied"),
            "the launch failure must preserve the OS cause: {stderr}"
        );
        Ok(())
    }
}
