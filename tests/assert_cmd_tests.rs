//! End-to-end CLI coverage for generated Ninja output.

use anyhow::{Context, Result, ensure};
use assert_cmd::Command;
use rstest::rstest;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{TempDir, tempdir};

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
    fs::copy("tests/data/minimal.yml", workdir.join("Netsukefile"))
        .with_context(|| format!("copy manifest for {context}"))?;
    Ok((temp, workdir))
}

fn create_netsuke_command(current_dir: &Path) -> Command {
    let mut command = assert_cmd::cargo::cargo_bin_cmd!("netsuke");
    command.current_dir(current_dir);
    command
}

#[rstest]
fn generate_writes_file() -> Result<()> {
    let temp = setup_simple_workspace("generate file test")?;
    let output = temp.path().join("standalone.ninja");

    create_netsuke_command(temp.path())
        .args(["generate", "--output"])
        .arg(&output)
        .assert()
        .success();

    ensure!(output.exists(), "generate should create the output file");
    Ok(())
}

#[rstest]
fn generate_streams_to_stdout_by_default() -> Result<()> {
    let temp = setup_simple_workspace("generate stdout test")?;
    let output = create_netsuke_command(temp.path())
        .arg("generate")
        .output()
        .context("run netsuke generate")?;
    ensure!(output.status.success(), "generate should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.contains("rule ") && stdout.contains("build "),
        "generate should print Ninja content, got: {stdout}"
    );
    Ok(())
}

#[rstest]
fn generate_resolves_output_relative_to_directory() -> Result<()> {
    let (temp, workdir) = setup_workspace_with_subdir("generate -C test")?;

    create_netsuke_command(temp.path())
        .args(["-C", "work", "generate", "--output", "out.ninja"])
        .assert()
        .success();

    ensure!(
        workdir.join("out.ninja").exists(),
        "generate output should be written relative to -C directory"
    );
    ensure!(
        !temp.path().join("out.ninja").exists(),
        "generate output should not be written outside -C directory"
    );
    Ok(())
}

#[rstest]
fn generate_streams_to_stdout_with_directory() -> Result<()> {
    let (temp, _workdir) = setup_workspace_with_subdir("generate stdout -C test")?;
    let output = create_netsuke_command(temp.path())
        .args(["-C", "work", "generate"])
        .output()
        .context("run netsuke -C work generate")?;
    ensure!(output.status.success(), "generate with -C should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.contains("rule ") && stdout.contains("build "),
        "generate with -C should print Ninja content, got: {stdout}"
    );
    Ok(())
}
