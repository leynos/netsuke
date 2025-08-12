//! Integration tests for CLI execution using `assert_cmd`.
//!
//! These tests exercise end-to-end command handling by invoking the compiled
//! binary and verifying file outputs for the `manifest` subcommand and the
//! `--emit` build option.

use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

#[expect(unused, reason = "support module exports helpers unused in this test")]
mod support;

#[test]
fn manifest_subcommand_writes_file() {
    let temp = tempdir().expect("temp dir");
    fs::copy("tests/data/minimal.yml", temp.path().join("Netsukefile")).expect("copy manifest");
    let output = temp.path().join("standalone.ninja");
    let mut cmd = Command::cargo_bin("netsuke").expect("binary");
    cmd.current_dir(temp.path())
        .env("PATH", "")
        .arg("manifest")
        .arg(&output)
        .assert()
        .success();
    assert!(output.exists());
}

#[test]
fn build_with_emit_writes_file() {
    let (ninja_dir, _ninja_path) = support::fake_ninja(0);
    let temp = tempdir().expect("temp dir");
    fs::copy("tests/data/minimal.yml", temp.path().join("Netsukefile")).expect("copy manifest");
    let output = temp.path().join("emitted.ninja");
    let original = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths(
        std::iter::once(ninja_dir.path().to_path_buf()).chain(std::env::split_paths(&original)),
    )
    .expect("join path");
    let mut cmd = Command::cargo_bin("netsuke").expect("binary");
    cmd.current_dir(temp.path())
        .env("PATH", path)
        .arg("build")
        .arg("--emit")
        .arg(&output)
        .assert()
        .success();
    assert!(output.exists());
}
