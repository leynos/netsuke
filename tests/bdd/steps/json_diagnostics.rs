//! Step definitions for JSON diagnostics behaviour.

use crate::bdd::fixtures::TestWorld;
use anyhow::{Context, Result, ensure};
use rstest_bdd_macros::then;
use serde_json::Value;

/// Assert that a captured output stream was recorded and is empty, sharing the
/// retrieval, capture, and emptiness checks between the stdout and stderr steps.
fn assert_captured_stream_is_empty(stream_name: &str, output: Option<&String>) -> Result<()> {
    let captured = output.with_context(|| format!("{stream_name} should be captured"))?;
    ensure!(
        captured.is_empty(),
        "expected {stream_name} to be empty, got:\n{captured}"
    );
    Ok(())
}

#[then("stdout should be empty")]
fn stdout_should_be_empty(world: &TestWorld) -> Result<()> {
    assert_captured_stream_is_empty("stdout", world.command_stdout.get().as_ref())
}

#[then("stderr should be empty")]
fn stderr_should_be_empty(world: &TestWorld) -> Result<()> {
    assert_captured_stream_is_empty("stderr", world.command_stderr.get().as_ref())
}

#[then("stderr should be valid diagnostics json")]
fn stderr_should_be_valid_diagnostics_json(world: &TestWorld) -> Result<()> {
    let stderr = world
        .command_stderr
        .get()
        .context("stderr should be captured")?;
    let value: Value = serde_json::from_str(&stderr).context("stderr should be valid JSON")?;
    let schema_version = value
        .get("schema_version")
        .and_then(Value::as_i64)
        .context("diagnostics JSON should include schema_version")?;
    let diagnostics = value
        .get("diagnostics")
        .and_then(Value::as_array)
        .context("diagnostics JSON should include a diagnostics array")?;
    ensure!(
        schema_version == 1,
        "diagnostics JSON should include the schema version",
    );
    ensure!(
        !diagnostics.is_empty(),
        "diagnostics JSON should include a diagnostics array",
    );
    Ok(())
}

/// Parse captured stdout as exactly one result JSON document, asserting its
/// schema version and command name, and return the `result` object.
fn parse_single_result_document(world: &TestWorld, command: &str) -> Result<Value> {
    let stdout = world
        .command_stdout
        .get()
        .context("stdout should be captured")?;
    let value: Value =
        serde_json::from_str(&stdout).context("stdout should be exactly one JSON document")?;
    ensure!(
        value.get("schema_version").and_then(Value::as_i64) == Some(1),
        "result JSON should include schema version 1"
    );
    ensure!(
        value.get("result").is_some() && value.get("diagnostics").is_none(),
        "result JSON should contain only the result branch",
    );
    let result = value
        .get("result")
        .context("result JSON should include a result object")?;
    ensure!(
        result.is_object(),
        "result JSON should include an object-valued result",
    );
    ensure!(
        result.get("command").and_then(Value::as_str) == Some(command),
        "result JSON should identify the {command} command"
    );
    Ok(result.clone())
}

#[then("stdout should be one generate result json document")]
fn stdout_should_be_one_generate_result_json_document(world: &TestWorld) -> Result<()> {
    let result = parse_single_result_document(world, "generate")?;
    let content = result
        .get("content")
        .and_then(Value::as_str)
        .context("generate result JSON should include generated content")?;
    ensure!(
        content.contains("rule ") && content.contains("build hello: "),
        "generate result should contain the Ninja manifest"
    );
    Ok(())
}

#[then("stdout should be one clean result json document")]
fn stdout_should_be_one_clean_result_json_document(world: &TestWorld) -> Result<()> {
    let result = parse_single_result_document(world, "clean")?;
    ensure!(
        matches!(result.get("content"), Some(Value::Null)),
        "clean result JSON should not carry generated content: {result}"
    );
    Ok(())
}

#[then("stderr diagnostics code should be {code:string}")]
fn stderr_diagnostics_code_should_be(world: &TestWorld, code: &str) -> Result<()> {
    let stderr = world
        .command_stderr
        .get()
        .context("stderr should be captured")?;
    let value: Value = serde_json::from_str(&stderr).context("stderr should be valid JSON")?;
    let diagnostic_code = value
        .get("diagnostics")
        .and_then(Value::as_array)
        .and_then(|diagnostics| diagnostics.first())
        .and_then(|diagnostic| diagnostic.get("code"))
        .and_then(Value::as_str)
        .context("first diagnostic should include a code")?;
    ensure!(
        diagnostic_code == code,
        "expected diagnostics code {code}, got {value}",
    );
    Ok(())
}
