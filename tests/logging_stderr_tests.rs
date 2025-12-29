//! Integration tests verifying that log output is written to stderr.
//!
//! These tests exercise the production logging path by invoking the compiled
//! binary and asserting log messages appear on stderr rather than stdout.

use anyhow::{Context, Result, ensure};
use tempfile::tempdir;

/// Verifies that runner errors are logged to stderr.
///
/// The test creates an empty temporary directory (no manifest) and runs the
/// `graph` subcommand, which fails quickly. The error log should appear on
/// stderr, not stdout.
#[test]
fn main_logs_errors_to_stderr() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("netsuke");
    let output = cmd
        .current_dir(temp.path())
        .arg("graph")
        .output()
        .context("run netsuke graph")?;

    ensure!(!output.status.success(), "command should fail without manifest");

    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains("runner failed"),
        "stderr should contain 'runner failed', got: {stderr}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        !stdout.contains("runner failed"),
        "stdout should not contain 'runner failed', got: {stdout}"
    );

    Ok(())
}
