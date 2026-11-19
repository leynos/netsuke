//! Shared process fixtures for the Ninja diagnostic suites.
//!
//! These helpers stage a workspace, run the real command line interface with
//! a child-only `PATH`, and read the rendered diagnostics back. The launch
//! cases live in `launch_diagnostics`, the failed-recipe cases in
//! `ninja_exit_diagnostics`, and both read the same fixtures so a change to
//! how Netsuke is invoked reaches every suite at once.

use super::create_netsuke_command;
use anyhow::{Context, Result, ensure};
use netsuke::runner::NINJA_ENV;
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
pub(super) fn missing_ninja_program() -> &'static str {
    if cfg!(windows) {
        ABSENT_WINDOWS_PROGRAM
    } else {
        "ninja"
    }
}

/// Supply the child-only override the Windows missing-Ninja cases require.
pub(super) fn missing_ninja_override() -> Option<&'static Path> {
    cfg!(windows).then_some(Path::new(ABSENT_WINDOWS_PROGRAM))
}

/// Text the platform's resolver reports for an absent bare program name.
///
/// Windows resolution reports a synthetic `NotFound` with no OS code, while
/// Unix passes through `execvp`'s `ENOENT` and its code.
pub(super) fn absent_program_cause() -> &'static str {
    if cfg!(windows) {
        "program not found"
    } else {
        // The parentheses keep the assertion from accepting a different
        // numbered OS error that merely contains `os error 2`.
        "(os error 2)"
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

/// A blocked `env()` lookup must not disclose the variable's name or value.
///
/// The rejection is deliberate: a manifest cannot use an operator-blocked
/// variable to reference a secret. A diagnostic that echoed the variable, or
/// the value the child process was given, would undo that protection at the
/// reporting boundary.
#[test]
fn blocked_environment_lookup_does_not_disclose_the_variable() -> Result<()> {
    const VARIABLE: &str = "NETSUKE_SECRET_VALUE";
    const SECRET: &str = "s3cr3t-value";
    let manifest = r#"netsuke_version: "1.0.0"
targets:
  - name: hello
    command: "echo {{ env('NETSUKE_SECRET_VALUE') }}"
"#;
    ensure!(
        manifest.contains(VARIABLE),
        "the test manifest must exercise the blocked variable"
    );
    let temp = tempdir().context("create blocked-environment workspace")?;
    fs::write(temp.path().join("Netsukefile"), manifest)
        .context("write blocked-environment manifest")?;

    let output = create_netsuke_command(temp.path())
        .env(VARIABLE, SECRET)
        .args(["--locale", "en-GB", "--env-block-var", VARIABLE, "generate"])
        .output()
        .context("run generate with a blocked environment lookup")?;
    ensure!(
        !output.status.success(),
        "a blocked environment lookup must fail generate, got status {:?}",
        output.status
    );
    let stderr = String::from_utf8(output.stderr).context("diagnostic stderr is UTF-8")?;
    ensure!(
        stderr.contains("Access to an environment variable is blocked"),
        "stderr must report the blocked lookup: {stderr}"
    );
    ensure!(
        !stderr.contains(VARIABLE),
        "stderr must not disclose the blocked variable's name: {stderr}"
    );
    ensure!(
        !stderr.contains(SECRET),
        "stderr must not disclose the blocked variable's value: {stderr}"
    );
    Ok(())
}

/// A permitted `env()` value must stay out of an interpolation diagnostic.
///
/// `env()` runs while the manifest is rendered, so an allowed value is already
/// inside the recipe text before interpolation validates it. Quoting the
/// rejected command would therefore publish the value in the default human
/// output, which is the disclosure the blocked-lookup test above cannot catch.
#[cfg(unix)]
#[test]
fn permitted_environment_value_stays_out_of_the_interpolation_diagnostic() -> Result<()> {
    const VARIABLE: &str = "NETSUKE_SECRET_VALUE";
    const SECRET: &str = "s3cr3t-value";
    let manifest = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: hello\n",
        "    command: \"echo {{ env('NETSUKE_SECRET_VALUE') }} `\"\n",
    );
    ensure!(
        manifest.contains(VARIABLE),
        "the test manifest must resolve a permitted variable"
    );
    let temp = tempdir().context("create permitted-environment workspace")?;
    fs::write(temp.path().join("Netsukefile"), manifest)
        .context("write permitted-environment manifest")?;

    let output = create_netsuke_command(temp.path())
        .env(VARIABLE, SECRET)
        .args(["--locale", "en-GB", "generate"])
        .output()
        .context("run generate with a permitted environment lookup")?;
    ensure!(
        !output.status.success(),
        "the unmatched backtick must fail generate, got status {:?}",
        output.status
    );
    let stderr = String::from_utf8(output.stderr).context("diagnostic stderr is UTF-8")?;
    ensure!(
        stderr.contains("Invalid command interpolation: unmatched backtick"),
        "stderr must name the failed syntax: {stderr}"
    );
    ensure!(
        !stderr.contains(SECRET),
        "stderr must not disclose the permitted variable's value: {stderr}"
    );
    ensure!(
        !stderr.contains(VARIABLE),
        "stderr must not repeat the manifest expression: {stderr}"
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
