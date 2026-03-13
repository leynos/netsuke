//! Step definitions for JSON diagnostics behaviour.

use crate::bdd::fixtures::TestWorld;
use anyhow::{Context, Result, ensure};
use rstest_bdd_macros::then;
use serde_json::Value;

#[then("stdout should be empty")]
fn stdout_should_be_empty(world: &TestWorld) -> Result<()> {
    let stdout = world
        .command_stdout
        .get()
        .context("stdout should be captured")?;
    ensure!(
        stdout.is_empty(),
        "expected stdout to be empty, got:\n{stdout}"
    );
    Ok(())
}

#[then("stderr should be empty")]
fn stderr_should_be_empty(world: &TestWorld) -> Result<()> {
    let stderr = world
        .command_stderr
        .get()
        .context("stderr should be captured")?;
    ensure!(
        stderr.is_empty(),
        "expected stderr to be empty, got:\n{stderr}"
    );
    Ok(())
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
