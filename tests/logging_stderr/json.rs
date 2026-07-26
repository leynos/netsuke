//! JSON diagnostic, result-envelope, and standard stderr integration tests.

use super::support::temp_with_minimal_manifest;
use anyhow::{Context, Result, ensure};
#[cfg(unix)]
use netsuke::runner::NINJA_ENV;
use predicates::prelude::*;
use rstest::rstest;
use serde_json::Value;
use std::fs;
use tempfile::{TempDir, tempdir};
#[cfg(unix)]
use test_support::check_ninja::{ToolName, fake_ninja_check_build_file, fake_ninja_expect_tool};

fn parse_success_result(output: std::process::Output, command: &str) -> Result<Value> {
    ensure!(
        output.status.success(),
        "expected {command} command success"
    );
    ensure!(
        output.stderr.is_empty(),
        "stderr should stay empty for successful {command} JSON output"
    );
    let stdout = String::from_utf8(output.stdout).context("stdout should be valid UTF-8")?;
    let document: Value =
        serde_json::from_str(&stdout).context("stdout should contain exactly one JSON document")?;
    ensure!(
        document.get("schema_version").and_then(Value::as_u64) == Some(1),
        "JSON result should use schema version 1: {document}"
    );
    ensure!(
        document.pointer("/result/command").and_then(Value::as_str) == Some(command),
        "JSON result should identify the {command} command: {document}"
    );
    Ok(document)
}

#[test]
fn main_logs_errors_to_stderr() {
    let temp = tempdir().expect("create temp dir");
    assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .arg("graph")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found in"))
        .stdout(predicate::str::contains("Netsukefile").not());
}

#[test]
fn json_failures_emit_single_json_document_on_stderr() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .arg("--json")
        .arg("graph")
        .output()
        .context("run netsuke with --json")?;

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
    let diagnostic_code = value
        .get("diagnostics")
        .and_then(Value::as_array)
        .and_then(|diagnostics| diagnostics.first())
        .and_then(|diagnostic| diagnostic.get("code"))
        .and_then(Value::as_str)
        .context("first diagnostic should include a code")?;
    ensure!(
        schema_version == 1,
        "JSON diagnostics should use schema version 1"
    );
    ensure!(
        diagnostic_code == "netsuke::runner::manifest_not_found",
        "missing manifest should map to the runner diagnostic code"
    );
    ensure!(
        !stderr.contains("ERROR"),
        "stderr should not contain tracing or text-mode prefixes: {stderr}"
    );
    Ok(())
}

#[rstest]
fn json_success_keeps_stdout_artefact_and_stderr_empty(
    temp_with_minimal_manifest: Result<TempDir>,
) -> Result<()> {
    let temp = temp_with_minimal_manifest?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .arg("--json")
        .arg("generate")
        .output()
        .context("run netsuke generate with --json")?;
    let document = parse_success_result(output, "generate")?;
    let ninja = document
        .pointer("/result/content")
        .and_then(Value::as_str)
        .context("generate result should contain the Ninja artefact")?;
    ensure!(
        ninja.contains("build hello: "),
        "generate result should contain the hello build edge: {ninja}"
    );
    Ok(())
}

/// A successful `--json build` emits exactly one build result document on
/// stdout with empty stderr. A fake Ninja that exits cleanly stands in for the
/// real build so the test stays deterministic.
#[cfg(unix)]
#[rstest]
fn json_success_build_keeps_stdout_result_and_stderr_empty(
    temp_with_minimal_manifest: Result<TempDir>,
) -> Result<()> {
    let temp = temp_with_minimal_manifest?;
    let (_ninja_dir, ninja) =
        fake_ninja_check_build_file().context("create fake ninja for build")?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .env(NINJA_ENV, &ninja)
        .arg("--json")
        .arg("build")
        .output()
        .context("run netsuke build with --json")?;
    let document = parse_success_result(output, "build")?;
    ensure!(
        document.pointer("/result/content") == Some(&Value::Null),
        "build result should not carry generated content: {document}"
    );
    Ok(())
}

/// A successful `--json clean` emits exactly one clean result document on stdout
/// with empty stderr, and drives the real Ninja clean tool. The fake Ninja
/// exits `0` only when invoked with `-t clean` and `-f <file>`, so a successful
/// result proves Netsuke ran the clean tool rather than merely printing JSON.
#[cfg(unix)]
#[rstest]
fn json_success_clean_invokes_clean_tool_with_empty_stderr(
    temp_with_minimal_manifest: Result<TempDir>,
) -> Result<()> {
    let temp = temp_with_minimal_manifest?;
    let (_ninja_dir, ninja) = fake_ninja_expect_tool(ToolName::new("clean"))
        .context("create fake ninja expecting -t clean")?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .env(NINJA_ENV, &ninja)
        .arg("--json")
        .arg("clean")
        .output()
        .context("run netsuke clean with --json")?;
    let document = parse_success_result(output, "clean")?;
    ensure!(
        document.pointer("/result/content") == Some(&Value::Null),
        "clean result should not carry generated content: {document}"
    );
    Ok(())
}

fn assert_json_passthrough(flag: &str, context: &str, stdout_marker: &str) -> Result<()> {
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .arg("--json")
        .arg(flag)
        .output()
        .with_context(|| format!("run netsuke --json {flag}"))?;

    ensure!(
        output.status.success(),
        "{context} should exit successfully"
    );
    let stdout = String::from_utf8(output.stdout).context("stdout should be valid UTF-8")?;
    ensure!(
        stdout.contains(stdout_marker),
        "{context} output should remain human-readable"
    );
    ensure!(
        output.stderr.is_empty(),
        "{context} should not emit diagnostics JSON"
    );
    Ok(())
}

#[rstest]
#[case("--help", "help", "Usage:")]
#[case("--version", "version", "netsuke")]
fn json_passthrough_uses_normal_clap_output(
    #[case] flag: &str,
    #[case] context: &str,
    #[case] stdout_marker: &str,
) -> Result<()> {
    assert_json_passthrough(flag, context, stdout_marker)
}

#[test]
fn config_driven_json_formats_merge_failures_as_json() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let config_path = temp.path().join("netsuke.toml");
    fs::write(&config_path, "json = true\njobs = \"many\"\n")
        .with_context(|| format!("write config {}", config_path.display()))?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .env("NETSUKE_CONFIG", &config_path)
        .output()
        .context("run netsuke with invalid config")?;

    ensure!(!output.status.success(), "expected merge failure");
    ensure!(
        output.stdout.is_empty(),
        "stdout should remain empty on merge failure"
    );
    let stderr = String::from_utf8(output.stderr).context("stderr should be valid UTF-8")?;
    let value: Value = serde_json::from_str(&stderr).context("stderr should be valid JSON")?;
    let message = value
        .get("diagnostics")
        .and_then(Value::as_array)
        .and_then(|diagnostics| diagnostics.first())
        .and_then(|diagnostic| diagnostic.get("message"))
        .and_then(Value::as_str)
        .context("first diagnostic should include a message")?;
    ensure!(
        message.contains("invalid type") && message.contains("expected usize"),
        "merge failure should describe the config type mismatch: {message}"
    );
    Ok(())
}

#[test]
fn json_flag_formats_config_load_failures_as_json() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let config_path = temp.path().join("broken.toml");
    fs::write(&config_path, "emoji = \"never\n")
        .with_context(|| format!("write malformed config {}", config_path.display()))?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .args(["--config", &config_path.to_string_lossy(), "--json"])
        .output()
        .context("run netsuke with malformed config and JSON output format")?;

    ensure!(!output.status.success(), "expected config load failure");
    ensure!(
        output.stdout.is_empty(),
        "stdout should remain empty on config load failure"
    );
    let stderr = String::from_utf8(output.stderr).context("stderr should be valid UTF-8")?;
    let value: Value = serde_json::from_str(&stderr).context("stderr should be valid JSON")?;
    ensure!(
        value.get("diagnostics").and_then(Value::as_array).is_some(),
        "JSON diagnostics should include a diagnostics array: {value:?}"
    );
    Ok(())
}

#[rstest]
fn json_success_graph_keeps_clean_stderr(
    temp_with_minimal_manifest: Result<TempDir>,
) -> Result<()> {
    let temp = temp_with_minimal_manifest?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .arg("--json")
        .arg("graph")
        .output()
        .context("run netsuke graph")?;
    let document = parse_success_result(output, "graph")?;
    let graph = document
        .pointer("/result/content")
        .and_then(Value::as_str)
        .context("graph result should contain the DOT artefact")?;
    ensure!(
        graph.contains("rankdir=\"LR\"")
            && graph.contains("\"hello\" [label=\"hello\"")
            && graph.lines().any(|line| line == "}"),
        "graph result should contain the expected hello-node structure: {graph}"
    );
    Ok(())
}
