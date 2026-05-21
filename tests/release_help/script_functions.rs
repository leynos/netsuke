//! Tests for source-safe helper functions in the release-help script.

use super::script_path;
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::{path::Path, process::Command};

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
