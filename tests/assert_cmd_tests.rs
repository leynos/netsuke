//! Integration tests for CLI execution using `assert_cmd`.
//!
//! These tests exercise end-to-end command handling by invoking the compiled
//! binary and verifying file outputs for the `manifest` subcommand and the
//! `--emit` build option.

use anyhow::{Context, Result, ensure};
use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;
use test_support::fake_ninja;

#[test]
fn manifest_subcommand_writes_file() -> Result<()> {
    let temp = tempdir().context("create temp dir for manifest test")?;
    let netsukefile = temp.path().join("Netsukefile");
    fs::copy("tests/data/minimal.yml", &netsukefile)
        .with_context(|| format!("copy manifest to {}", netsukefile.display()))?;
    let output = temp.path().join("standalone.ninja");
    let mut cmd = Command::cargo_bin("netsuke").context("locate netsuke binary")?;
    cmd.current_dir(temp.path())
        .env("PATH", "")
        .arg("manifest")
        .arg(&output)
        .assert()
        .success();
    ensure!(
        output.exists(),
        "manifest command should create output file"
    );
    Ok(())
}

#[test]
fn build_with_emit_writes_file() -> Result<()> {
    let (ninja_dir, _ninja_path) = fake_ninja(0u8);
    let temp = tempdir().context("create temp dir for build test")?;
    let netsukefile = temp.path().join("Netsukefile");
    fs::copy("tests/data/minimal.yml", &netsukefile)
        .with_context(|| format!("copy manifest to {}", netsukefile.display()))?;
    let output = temp.path().join("emitted.ninja");
    let original = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths(
        std::iter::once(ninja_dir.path().to_path_buf()).chain(std::env::split_paths(&original)),
    )
    .context("construct PATH with fake ninja")?;
    let mut cmd = Command::cargo_bin("netsuke").context("locate netsuke binary")?;
    cmd.current_dir(temp.path())
        .env("PATH", path)
        .arg("build")
        .arg("--emit")
        .arg(&output)
        .assert()
        .success();
    ensure!(
        output.exists(),
        "build --emit should create emitted manifest"
    );
    Ok(())
}
