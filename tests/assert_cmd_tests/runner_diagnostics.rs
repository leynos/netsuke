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

/// Program name a Windows fixture lets Netsuke attempt.
///
/// Windows resolves a bare name beside `netsuke.exe` and on the parent
/// process `PATH` as well as on the child `PATH`, and a test controls only the
/// last. A host Ninja would therefore turn the missing-Ninja cases into real
/// builds. The unique name is absent everywhere, so the launch still fails
/// deterministically; the default program selection itself is covered by the
/// `runner::process` unit tests.
const ABSENT_WINDOWS_PROGRAM: &str = "netsuke-fixture-absent-ninja";

/// Name the program the missing-Ninja fixtures let Netsuke attempt.
fn missing_ninja_program() -> &'static str {
    if cfg!(windows) {
        ABSENT_WINDOWS_PROGRAM
    } else {
        "ninja"
    }
}

/// Supply the child-only override the Windows missing-Ninja cases require.
fn missing_ninja_override() -> Option<&'static Path> {
    cfg!(windows).then_some(Path::new(ABSENT_WINDOWS_PROGRAM))
}

/// Text the platform's resolver reports for an absent bare program name.
///
/// Windows resolution reports a synthetic `NotFound` with no OS code, while
/// Unix passes through `execvp`'s `ENOENT` and its code.
fn absent_program_cause() -> &'static str {
    if cfg!(windows) {
        "program not found"
    } else {
        "os error 2"
    }
}

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

/// Whether text names the generated `netsuke.<random>.ninja` build file.
///
/// The `netsuke.` prefix alone is not enough: a temporary directory may
/// legitimately be called `netsuke.review`, and the diagnostic has to name
/// that path. The generated file is a path whose final component starts with
/// `netsuke.` and ends with `.ninja`.
pub(super) fn names_generated_build_file(text: &str) -> bool {
    text.split(|character: char| character.is_whitespace() || character == '`')
        .map(|token| {
            token.trim_matches(|character: char| {
                matches!(
                    character,
                    '\'' | '"' | ',' | ';' | ':' | '(' | ')' | '[' | ']'
                )
            })
        })
        .any(|token| {
            token.contains("netsuke.")
                && Path::new(token)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("ninja"))
        })
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
    let expected =
        flatten_localized_text(&format!("Could not start the Ninja program `{program}`."));
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

/// Assert the JSON launch diagnostic for `program`, caused by `launch_cause`.
fn assert_spawn_failure_json(stderr: &str, program: &str, launch_cause: &str) -> Result<()> {
    let diagnostic = first_json_diagnostic(stderr)?;
    ensure!(
        string_field(&diagnostic, "code")? == "netsuke::runner::ninja_spawn_failed",
        "the JSON diagnostic must carry its stable code: {stderr}"
    );
    let expected =
        flatten_localized_text(&format!("Could not start the Ninja program `{program}`."));
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
            .any(|cause| cause.contains(launch_cause)),
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
    let program = missing_ninja_program();
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
    assert_spawn_failure_reported(&stderr, program)
}

/// Report the launch failure in every rendering mode of both subcommands.
///
/// The human `build --progress never` case is owned by
/// [`missing_ninja_reports_the_documented_summary`], which the issue names, so
/// this matrix does not repeat it.
#[rstest]
#[case("build", "always", false)]
#[case("build", "always", true)]
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
    let output = run_netsuke(
        temp.path(),
        empty_path.path(),
        missing_ninja_override(),
        &arguments,
    )?;
    ensure!(
        !output.status.success(),
        "{subcommand} must fail without an installed Ninja"
    );
    if json {
        ensure!(
            output.stdout.is_empty(),
            "a failing JSON run must not write to stdout: {:?}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
    let stderr = String::from_utf8(output.stderr).context("diagnostic stderr is UTF-8")?;
    let program = missing_ninja_program();
    if json {
        assert_spawn_failure_json(&stderr, program, absent_program_cause())
    } else {
        assert_spawn_failure_reported(&stderr, program)
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

/// Report a Ninja program that exists but cannot be executed, in every mode.
#[cfg(unix)]
#[rstest]
#[case("build", "always", false)]
#[case("build", "always", true)]
#[case("build", "never", false)]
#[case("build", "never", true)]
#[case("clean", "always", false)]
#[case("clean", "always", true)]
#[case("clean", "never", false)]
#[case("clean", "never", true)]
fn non_executable_ninja_is_reported_as_a_launch_failure(
    #[case] subcommand: &str,
    #[case] progress: &str,
    #[case] json: bool,
) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let temp = workspace_with_minimal_manifest("a non-executable Ninja")?;
    let empty_path = tempdir().context("create an empty PATH directory")?;
    let script = temp.path().join("ninja-not-executable");
    fs::write(&script, "#!/bin/sh\nexit 0\n").context("write the non-executable Ninja file")?;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o644))
        .context("remove the execute bit from the Ninja file")?;

    let mut arguments = vec!["--progress", progress];
    if json {
        arguments.push("--json");
    }
    arguments.push(subcommand);
    let output = run_netsuke(temp.path(), empty_path.path(), Some(&script), &arguments)?;
    ensure!(
        !output.status.success(),
        "{subcommand} with a non-executable Ninja must fail"
    );
    if json {
        ensure!(
            output.stdout.is_empty(),
            "a failing JSON run must not write to stdout: {:?}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
    let stderr = String::from_utf8(output.stderr).context("diagnostic stderr is UTF-8")?;
    let program = script.to_str().context("the configured path is UTF-8")?;
    if json {
        assert_spawn_failure_json(&stderr, program, "Permission denied")
    } else {
        assert_spawn_failure_reported(&stderr, program)?;
        ensure!(
            flatten_localized_text(&stderr).contains("Permission denied"),
            "the launch failure must preserve the OS cause: {stderr}"
        );
        Ok(())
    }
}
