//! Integration tests verifying that log output is written to stderr.
//!
//! These tests exercise the production logging path by invoking the compiled
//! binary and asserting log messages appear on stderr rather than stdout.

use predicates::prelude::*;
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
