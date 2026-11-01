//! Tests for source-safe helper functions in the release-help script.

use super::script_path;
use anyhow::{Context, Result, ensure};
use proptest::prelude::*;
use rstest::rstest;
use std::{fs, path::Path, process::Command};
use time::OffsetDateTime;

fn shell_quote_path(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

fn run_script_function(command: &str) -> Result<std::process::Output> {
    Command::new("bash")
        .arg("-c")
        .arg(format!(
            "source {}; {command}",
            shell_quote_path(&script_path())
        ))
        .env_remove("GITHUB_RUN_ID")
        .env_remove("GITHUB_RUN_ATTEMPT")
        .env_remove("RELEASE_HELP_BUILD_ID")
        .env_remove("RELEASE_HELP_CORRELATION_ID")
        .output()
        .context("run sourced release help script function")
}

#[rstest]
#[case(
    "PATH=/no-python-here SOURCE_DATE_EPOCH=1 manual_date",
    "1970-01-01",
    "Python is unavailable"
)]
#[case(
    "SOURCE_DATE_EPOCH=999999999999999999999999999999999999999999 manual_date",
    "1970-01-01",
    "is not a valid Unix timestamp"
)]
fn manual_date_falls_back_for_unconvertible_timestamps(
    #[case] command: &str,
    #[case] expected_stdout: &str,
    #[case] expected_warning: &str,
) -> Result<()> {
    let output = run_script_function(command)?;

    ensure!(
        output.status.success(),
        "manual_date should fall back: {output:?}"
    );
    let stdout = String::from_utf8(output.stdout).context("stdout should be UTF-8")?;
    ensure!(
        stdout.trim() == expected_stdout,
        "expected fallback date {expected_stdout}, got {stdout}"
    );
    let stderr = String::from_utf8(output.stderr).context("stderr should be UTF-8")?;
    ensure!(
        stderr.contains(expected_warning),
        "expected warning {expected_warning:?}, got {stderr}"
    );
    Ok(())
}

proptest! {
    #[test]
    fn manual_date_matches_utc_calendar_date_for_i32_epoch_range(epoch in any::<i32>()) {
        let output = run_script_function(&format!("SOURCE_DATE_EPOCH={epoch} manual_date"))
            .map_err(|err| TestCaseError::fail(err.to_string()))?;

        prop_assert!(
            output.status.success(),
            "manual_date should accept i32 Unix timestamps: {output:?}"
        );
        let stdout = std::str::from_utf8(&output.stdout)
            .map_err(|err| TestCaseError::fail(err.to_string()))?;
        let expected = OffsetDateTime::from_unix_timestamp(i64::from(epoch))
            .map_err(|err| TestCaseError::fail(err.to_string()))?
            .date()
            .to_string();

        prop_assert_eq!(stdout.trim(), expected);
    }
}

#[test]
fn annotation_escape_escapes_github_annotation_control_characters() -> Result<()> {
    let output = run_script_function("annotation_escape $'a%b\\r\\nc'")?;

    ensure!(
        output.status.success(),
        "annotation_escape failed: {output:?}"
    );
    let stdout = String::from_utf8(output.stdout).context("stdout should be UTF-8")?;
    ensure!(
        stdout.trim() == "a%25b%0D%0Ac",
        "annotation escaping should preserve GitHub annotation syntax, got {stdout}"
    );
    Ok(())
}

#[test]
fn require_file_formats_missing_file_errors() -> Result<()> {
    let output = run_script_function(
        "require_file /tmp/netsuke-release-help-missing-file 'manual page was not generated'",
    )?;

    ensure!(
        output.status.code() == Some(1),
        "missing file should exit 1, got {output:?}"
    );
    let stderr = String::from_utf8(output.stderr).context("stderr should be UTF-8")?;
    ensure!(
        stderr.contains("::error title=Release help missing::"),
        "missing file error should be annotation formatted, got {stderr}"
    );
    ensure!(
        stderr.contains("manual page was not generated: /tmp/netsuke-release-help-missing-file"),
        "missing file error should describe the missing path, got {stderr}"
    );
    ensure!(
        stderr.contains("build_id=local-0"),
        "missing file annotation should include build id, got {stderr}"
    );
    Ok(())
}

#[test]
fn require_file_rejects_empty_outputs() -> Result<()> {
    let output = run_script_function(
        "empty=$(mktemp); require_file \"$empty\" 'manual page was not generated'",
    )?;

    ensure!(
        output.status.code() == Some(1),
        "empty file should exit 1, got {output:?}"
    );
    let stderr = String::from_utf8(output.stderr).context("stderr should be UTF-8")?;
    ensure!(
        stderr.contains("Release help empty"),
        "empty file error should be annotation formatted, got {stderr}"
    );
    ensure!(
        stderr.contains("size_bytes=0"),
        "empty file error should report size, got {stderr}"
    );
    Ok(())
}

#[test]
fn require_file_logs_non_empty_output_size() -> Result<()> {
    let output = run_script_function(
        "file=$(mktemp); printf abc >\"$file\"; require_file \"$file\" 'manual page was not generated'",
    )?;

    ensure!(
        output.status.success(),
        "non-empty file should pass validation, got {output:?}"
    );
    let stderr = String::from_utf8(output.stderr).context("stderr should be UTF-8")?;
    ensure!(
        stderr.contains("Release help output"),
        "validated file should emit a notice, got {stderr}"
    );
    ensure!(
        stderr.contains("size_bytes=3"),
        "validated file notice should report size, got {stderr}"
    );
    Ok(())
}

/// Excerpt of the manual page `cargo-orthohelp` 0.10.0 renders.
///
/// The spellings are the renderer's own, not the fake generator's: hyphens are
/// escaped as `\-` and flags, value names and possible values carry roff font
/// escapes. The excerpt exercises the script's verification against that shape.
const RENDERED_MAN_EXCERPT: &str = concat!(
    ".SH SYNOPSIS\n",
    "\\fB\\-\\-no-input\\fR\n",
    "\\fB\\-\\-color\\fR \\fIPOLICY\\fR\n",
    ".SH OPTIONS\n",
    ".TP\n",
    "\\fB\\-\\-directory\\fR \\fIDIR\\fR, \\fB\\-C\\fR \\fIDIR\\fR\n",
    ".TP\n",
    "\\fB\\-\\-config\\fR \\fIFILE\\fR\n",
    ".TP\n",
    "\\fB\\-\\-default-target\\fR \\fITARGET\\fR\n",
    ".TP\n",
    "\\fB\\-\\-emoji\\fR \\fIPOLICY\\fR\n",
    "Possible values: \\fIauto, always, never\\fR\n",
    ".TP\n",
    "\\fB\\-\\-progress\\fR \\fIPOLICY\\fR\n",
    "Possible values: \\fIauto, always, never\\fR\n",
    ".TP\n",
    "\\fB\\-\\-accessibility\\fR \\fIPOLICY\\fR\n",
    "Possible values: \\fIauto, on, off\\fR\n",
);

/// Run a sourced release-help function against a document written to disk.
fn run_script_function_on_document(contents: &str, function: &str) -> Result<std::process::Output> {
    let directory = tempfile::tempdir().context("create rendered-help tempdir")?;
    let path = directory.path().join("rendered-help.txt");
    fs::write(&path, contents).context("write rendered help document")?;
    run_script_function(&format!("{function} {}", shell_quote_path(&path)))
}

#[test]
fn normalise_help_text_reduces_roff_escapes_to_plain_flags() -> Result<()> {
    let output = run_script_function_on_document(RENDERED_MAN_EXCERPT, "normalise_help_text")?;

    ensure!(
        output.status.success(),
        "normalise_help_text should succeed: {output:?}"
    );
    let text = String::from_utf8(output.stdout).context("stdout should be UTF-8")?;
    ensure!(
        text.contains("--color POLICY"),
        "roff font escapes should be stripped, got {text}"
    );
    // The separator rule that turns `auto, always, never` into one value set
    // also tightens the short-flag separator, so the comma has no space.
    ensure!(
        text.contains("--directory DIR,-C DIR"),
        "escaped hyphens should become plain hyphens, got {text}"
    );
    ensure!(
        text.contains("auto,always,never"),
        "possible values should be normalised into one comma-separated set, got {text}"
    );
    Ok(())
}

#[test]
fn verify_rendered_surface_accepts_a_real_renderer_manual_page() -> Result<()> {
    let output = run_script_function_on_document(RENDERED_MAN_EXCERPT, "verify_rendered_surface")?;

    ensure!(
        output.status.success(),
        "the reconciled surface should be accepted: {output:?}"
    );
    let stderr = String::from_utf8(output.stderr).context("stderr should be UTF-8")?;
    ensure!(
        stderr.contains("Release help surface"),
        "a verified document should emit a notice, got {stderr}"
    );
    Ok(())
}

#[rstest]
#[case("no-input", "omitted", "omits --no-input")]
#[case("auto, on, off", "auto on off", "omits the auto,on,off value set")]
#[case(
    ".SH OPTIONS",
    ".B \\-\\-default-targets\n.SH OPTIONS",
    "advertises the rejected --default-targets spelling"
)]
fn verify_rendered_surface_rejects_a_lost_or_invented_surface(
    #[case] rendered: &str,
    #[case] replacement: &str,
    #[case] expected_error: &str,
) -> Result<()> {
    let document = RENDERED_MAN_EXCERPT.replace(rendered, replacement);
    let output = run_script_function_on_document(&document, "verify_rendered_surface")?;

    ensure!(
        output.status.code() == Some(1),
        "a document losing {rendered:?} should be rejected: {output:?}"
    );
    let stderr = String::from_utf8(output.stderr).context("stderr should be UTF-8")?;
    ensure!(
        stderr.contains(expected_error),
        "expected {expected_error:?}, got {stderr}"
    );
    Ok(())
}
