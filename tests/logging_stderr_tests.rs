//! Integration tests verifying that log output is written to stderr.
//!
//! These tests exercise the production logging path by invoking the compiled
//! binary and asserting log messages appear on stderr rather than stdout.

use anyhow::{Context, Result, ensure};
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use tempfile::tempdir;

/// Verifies that runner errors are logged to stderr.
///
/// The test creates an empty temporary directory (no manifest) and runs the
/// `graph` subcommand, which fails quickly. The error log should appear on
/// stderr, not stdout.
#[test]
fn main_logs_errors_to_stderr() {
    let temp = tempdir().expect("create temp dir");
    // ManifestNotFound errors are rendered via miette with diagnostic output.
    assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .arg("graph")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found in"))
        .stdout(predicate::str::contains("Netsukefile").not());
}

#[test]
fn diag_json_failures_emit_single_json_document_on_stderr() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .arg("--diag-json")
        .arg("graph")
        .output()
        .context("run netsuke with --diag-json")?;

    ensure!(!output.status.success(), "expected command failure");
    ensure!(
        output.stdout.is_empty(),
        "stdout should remain empty on failure"
    );

    let stderr = String::from_utf8(output.stderr).context("stderr should be valid UTF-8")?;
    let value: Value = serde_json::from_str(&stderr).context("stderr should be valid JSON")?;
    let schema_version = value
        .get("schema_version")
        .and_then(Value::as_i64)
        .context("JSON diagnostics should include schema_version")?;
    let diagnostics = value
        .get("diagnostics")
        .and_then(Value::as_array)
        .context("JSON diagnostics should include a diagnostics array")?;
    let diagnostic_code = diagnostics
        .first()
        .and_then(|diagnostic| diagnostic.get("code"))
        .and_then(Value::as_str)
        .context("first diagnostic should include a code")?;
    ensure!(
        schema_version == 1,
        "JSON diagnostics should include the schema version",
    );
    ensure!(
        diagnostic_code == "netsuke::runner::manifest_not_found",
        "missing manifest should map to the runner diagnostic code",
    );
    ensure!(
        !stderr.contains("ERROR"),
        "stderr should not contain tracing or text-mode prefixes: {stderr}",
    );
    Ok(())
}

#[test]
fn diag_json_success_keeps_stdout_artifact_and_stderr_empty() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let manifest_path = temp.path().join("Netsukefile");
    fs::copy("tests/data/minimal.yml", &manifest_path)
        .with_context(|| format!("copy manifest to {}", manifest_path.display()))?;

    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .arg("--diag-json")
        .arg("manifest")
        .arg("-")
        .output()
        .context("run netsuke manifest with --diag-json")?;

    ensure!(output.status.success(), "expected command success");
    ensure!(
        output.stderr.is_empty(),
        "stderr should remain empty on success"
    );

    let stdout = String::from_utf8(output.stdout).context("stdout should be valid UTF-8")?;
    ensure!(
        stdout.contains("build hello: "),
        "stdout should contain the generated Ninja manifest, got:\n{stdout}",
    );
    Ok(())
}
