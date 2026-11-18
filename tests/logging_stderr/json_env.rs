//! Binary coverage for the strict `NETSUKE_JSON` environment spellings.
//!
//! The unit and merge tests prove each pass reads `1` and `0` as Booleans. These
//! tests prove the two passes agree in the real executable: `1` selects the JSON
//! envelope, and `0` keeps human-readable output.
//!
//! Both runs use [`run_netsuke_in_with_env`], which clears the inherited
//! environment and supplies only an isolated `PATH`, `HOME`, and
//! `XDG_CONFIG_HOME`. A developer's exported `NETSUKE_*` values or user
//! configuration therefore cannot change which spelling is being tested.

use super::support::temp_with_minimal_manifest;
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use serde_json::Value;
use tempfile::TempDir;
use test_support::netsuke::{NetsukeRun, run_netsuke_in_with_env};

/// Assert the run succeeded and returned exactly one `command` result envelope.
fn parse_success_envelope(run: &NetsukeRun, command: &str) -> Result<Value> {
    ensure!(run.success, "expected {command} command success");
    ensure!(
        run.stderr.is_empty(),
        "stderr should stay empty for successful {command} JSON output: {}",
        run.stderr
    );
    let document: Value =
        serde_json::from_str(&run.stdout).context("stdout should hold one JSON document")?;
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

/// `NETSUKE_JSON=1` selects the same JSON envelope as `--json`.
#[rstest]
fn environment_json_one_selects_the_json_envelope(
    temp_with_minimal_manifest: Result<TempDir>,
) -> Result<()> {
    let temp = temp_with_minimal_manifest?;
    let run = run_netsuke_in_with_env(temp.path(), &["graph"], &[("NETSUKE_JSON", "1")])
        .context("run netsuke graph with NETSUKE_JSON=1")?;
    let document = parse_success_envelope(&run, "graph")?;
    ensure!(
        document
            .pointer("/result/content")
            .is_some_and(|content| content
                .as_str()
                .is_some_and(|graph| graph.contains("rankdir=\"LR\""))),
        "NETSUKE_JSON=1 should emit the DOT artefact in the JSON envelope: {document}"
    );
    Ok(())
}

/// `NETSUKE_JSON=0` keeps human-readable output.
///
/// Human-readable mode still reports stage progress on stderr, so only stdout
/// distinguishes the two modes: human mode writes the DOT graph itself, while
/// JSON mode writes one result document.
#[rstest]
fn environment_json_zero_keeps_human_readable_output(
    temp_with_minimal_manifest: Result<TempDir>,
) -> Result<()> {
    let temp = temp_with_minimal_manifest?;
    let run = run_netsuke_in_with_env(temp.path(), &["graph"], &[("NETSUKE_JSON", "0")])
        .context("run netsuke graph with NETSUKE_JSON=0")?;
    ensure!(run.success, "NETSUKE_JSON=0 graph should succeed");
    ensure!(
        serde_json::from_str::<Value>(&run.stdout).is_err(),
        "NETSUKE_JSON=0 should not emit a JSON envelope: {}",
        run.stdout
    );
    ensure!(
        run.stdout.contains("rankdir=\"LR\"") && run.stdout.contains("\"hello\""),
        "NETSUKE_JSON=0 should emit the human-readable DOT graph: {}",
        run.stdout
    );
    Ok(())
}
