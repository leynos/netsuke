//! End-to-end rejection coverage for non-UTF-8 Ninja invocation paths.
//!
//! These tests pass raw operating-system strings to the built `netsuke`
//! binary. They verify that `clap` rejects invalid path bytes before
//! configuration discovery or the Ninja runner can start.

#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use assert_cmd::cargo::cargo_bin_cmd;
use rstest::rstest;
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use tempfile::tempdir;

/// Reject a non-UTF-8 Ninja invocation path before downstream startup work.
#[rstest]
#[case::file("--file", "Manifest path")]
#[case::directory("--directory", "Working directory path")]
fn non_utf8_ninja_path_fails_before_configuration_or_runner(
    #[case] flag: &str,
    #[case] diagnostic_subject: &str,
) -> Result<()> {
    let invocation_directory = tempdir().context("create empty invocation directory")?;
    let output = cargo_bin_cmd!("netsuke")
        .current_dir(invocation_directory.path())
        .env_clear()
        .arg(flag)
        .arg(OsString::from_vec(b"manifest-\xff".to_vec()))
        .output()
        .context("run netsuke with a non-UTF-8 Ninja path")?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    ensure!(
        !output.status.success(),
        "{flag} should reject non-UTF-8 input"
    );
    ensure!(
        stderr.contains(diagnostic_subject)
            && stderr.contains("not valid UTF-8")
            && stderr.contains("manifest-"),
        "{flag} should render its localized UTF-8 diagnostic, got: {stderr}"
    );
    ensure!(
        !stderr.contains("Netsukefile") && !stderr.contains("Ninja executable"),
        "{flag} should fail before configuration or runner setup, got: {stderr}"
    );
    ensure!(
        output.stdout.is_empty(),
        "{flag} should not emit a command result before parsing fails"
    );
    Ok(())
}
