//! Integration tests for CLI execution using `assert_cmd`.
//!
//! These tests exercise end-to-end command handling by invoking the compiled
//! binary and verifying file outputs for the `manifest` subcommand and the
//! `--emit` build option.

use anyhow::{Context, Result, ensure};
use assert_cmd::Command;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{TempDir, tempdir};
use test_support::fake_ninja;

fn path_with_fake_ninja(ninja_dir: &tempfile::TempDir) -> Result<OsString> {
    let original = std::env::var_os("PATH").unwrap_or_default();
    std::env::join_paths(
        std::iter::once(ninja_dir.path().to_path_buf()).chain(std::env::split_paths(&original)),
    )
    .context("construct PATH with fake ninja")
}

fn setup_simple_workspace(context: &str) -> Result<TempDir> {
    let temp = tempdir().with_context(|| format!("create temp dir for {context}"))?;
    let netsukefile = temp.path().join("Netsukefile");
    fs::copy("tests/data/minimal.yml", &netsukefile)
        .with_context(|| format!("copy manifest to {} for {context}", netsukefile.display()))?;
    Ok(temp)
}

fn setup_workspace_with_subdir(context: &str) -> Result<(TempDir, PathBuf)> {
    let temp = tempdir().with_context(|| format!("create temp dir for {context}"))?;
    let workdir = temp.path().join("work");
    fs::create_dir_all(&workdir).with_context(|| format!("create work directory for {context}"))?;
    let netsukefile = workdir.join("Netsukefile");
    fs::copy("tests/data/minimal.yml", &netsukefile)
        .with_context(|| format!("copy manifest to {} for {context}", netsukefile.display()))?;
    Ok((temp, workdir))
}

fn create_netsuke_command(current_dir: &Path, path_override: OsString) -> Result<Command> {
    let mut cmd = Command::cargo_bin("netsuke").context("locate netsuke binary")?;
    cmd.current_dir(current_dir).env("PATH", path_override);
    Ok(cmd)
}

#[test]
fn manifest_subcommand_writes_file() -> Result<()> {
    let temp = setup_simple_workspace("manifest test")?;
    let output = temp.path().join("standalone.ninja");
    let mut cmd = create_netsuke_command(temp.path(), OsString::from(""))?;
    cmd.arg("manifest").arg(&output).assert().success();
    ensure!(
        output.exists(),
        "manifest command should create output file"
    );
    Ok(())
}

#[test]
fn manifest_subcommand_streams_to_stdout_when_dash() -> Result<()> {
    let temp = tempdir().context("create temp dir for manifest stdout test")?;
    let netsukefile = temp.path().join("Netsukefile");
    fs::copy("tests/data/minimal.yml", &netsukefile)
        .with_context(|| format!("copy manifest to {}", netsukefile.display()))?;

    let mut cmd = Command::cargo_bin("netsuke").context("locate netsuke binary")?;
    let output = cmd
        .current_dir(temp.path())
        .env("PATH", "")
        .arg("manifest")
        .arg("-")
        .output()
        .context("run netsuke manifest -")?;
    ensure!(output.status.success(), "manifest - should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.contains("rule ") && stdout.contains("build "),
        "manifest - should print Ninja content, got: {stdout}"
    );
    ensure!(
        !temp.path().join("-").exists(),
        "manifest - should not create a file named '-'"
    );
    Ok(())
}

#[test]
fn manifest_subcommand_resolves_output_relative_to_directory() -> Result<()> {
    let (temp, workdir) = setup_workspace_with_subdir("manifest -C test")?;

    let mut cmd = create_netsuke_command(temp.path(), OsString::from(""))?;
    cmd.arg("-C")
        .arg("work")
        .arg("manifest")
        .arg("out.ninja")
        .assert()
        .success();

    ensure!(
        workdir.join("out.ninja").exists(),
        "manifest output should be written relative to -C directory"
    );
    ensure!(
        !temp.path().join("out.ninja").exists(),
        "manifest output should not be written outside -C directory"
    );
    Ok(())
}

#[test]
fn manifest_subcommand_streams_to_stdout_when_dash_with_directory() -> Result<()> {
    let temp = tempdir().context("create temp dir for manifest stdout -C test")?;
    let workdir = temp.path().join("work");
    fs::create_dir_all(&workdir).context("create work directory")?;
    let netsukefile = workdir.join("Netsukefile");
    fs::copy("tests/data/minimal.yml", &netsukefile)
        .with_context(|| format!("copy manifest to {}", netsukefile.display()))?;

    let mut cmd = Command::cargo_bin("netsuke").context("locate netsuke binary")?;
    let output = cmd
        .current_dir(temp.path())
        .env("PATH", "")
        .arg("-C")
        .arg("work")
        .arg("manifest")
        .arg("-")
        .output()
        .context("run netsuke -C work manifest -")?;
    ensure!(output.status.success(), "manifest - with -C should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.contains("rule ") && stdout.contains("build "),
        "manifest - with -C should print Ninja content, got: {stdout}"
    );
    ensure!(
        !temp.path().join("-").exists(),
        "manifest - with -C should not create a file named '-' in the working directory"
    );
    ensure!(
        !workdir.join("-").exists(),
        "manifest - with -C should not create a file named '-' in the -C directory"
    );
    Ok(())
}

#[test]
fn build_with_emit_writes_file() -> Result<()> {
    let (ninja_dir, _ninja_path) = fake_ninja(0u8)?;
    let temp = setup_simple_workspace("build test")?;
    let output = temp.path().join("emitted.ninja");
    let path = path_with_fake_ninja(&ninja_dir)?;
    let mut cmd = create_netsuke_command(temp.path(), path)?;
    cmd.arg("build")
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

#[test]
fn build_with_emit_resolves_output_relative_to_directory() -> Result<()> {
    let (ninja_dir, _ninja_path) = fake_ninja(0u8)?;
    let (temp, workdir) = setup_workspace_with_subdir("build -C test")?;

    let path = path_with_fake_ninja(&ninja_dir)?;
    let mut cmd = create_netsuke_command(temp.path(), path)?;
    cmd.arg("-C")
        .arg("work")
        .arg("build")
        .arg("--emit")
        .arg("emitted.ninja")
        .assert()
        .success();

    ensure!(
        workdir.join("emitted.ninja").exists(),
        "build --emit should write output relative to -C directory"
    );
    ensure!(
        !temp.path().join("emitted.ninja").exists(),
        "build --emit should not write output outside -C directory"
    );
    Ok(())
}
