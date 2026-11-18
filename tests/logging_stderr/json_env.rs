//! Binary coverage for the strict `NETSUKE_JSON` environment spellings.
//!
//! The unit and merge tests prove each pass reads `1` and `0` as Booleans. These
//! tests prove the two passes agree in the real executable: `1` selects the JSON
//! envelope, and `0` keeps human-readable output.

use super::json::parse_success_result;
use super::support::temp_with_minimal_manifest;
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use serde_json::Value;
use tempfile::TempDir;

/// `NETSUKE_JSON=1` selects the same JSON envelope as `--json`.
#[rstest]
fn environment_json_one_selects_the_json_envelope(
    temp_with_minimal_manifest: Result<TempDir>,
) -> Result<()> {
    let temp = temp_with_minimal_manifest?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .env("NETSUKE_JSON", "1")
        .arg("graph")
        .output()
        .context("run netsuke graph with NETSUKE_JSON=1")?;
    let document = parse_success_result(output, "graph")?;
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
#[rstest]
fn environment_json_zero_keeps_human_readable_output(
    temp_with_minimal_manifest: Result<TempDir>,
) -> Result<()> {
    let temp = temp_with_minimal_manifest?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .env("NETSUKE_JSON", "0")
        .arg("graph")
        .output()
        .context("run netsuke graph with NETSUKE_JSON=0")?;
    ensure!(
        output.status.success(),
        "NETSUKE_JSON=0 graph should succeed"
    );
    ensure!(
        output.stderr.is_empty(),
        "NETSUKE_JSON=0 should keep stderr empty on success"
    );
    let stdout = String::from_utf8(output.stdout).context("stdout should be valid UTF-8")?;
    ensure!(
        serde_json::from_str::<Value>(&stdout).is_err(),
        "NETSUKE_JSON=0 should not emit a JSON envelope: {stdout}"
    );
    ensure!(
        stdout.contains("rankdir=\"LR\"") && stdout.contains("\"hello\""),
        "NETSUKE_JSON=0 should emit the human-readable DOT graph: {stdout}"
    );
    Ok(())
}
