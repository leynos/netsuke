//! End-to-end configuration discovery failure coverage.
//!
//! These tests run the real binary in a child process with a closed
//! environment, proving that missing configuration still permits the normal
//! workflow while a malformed discovered file fails before manifest handling.

use anyhow::{Context, Result, ensure};
use assert_cmd::cargo::cargo_bin_cmd;
use tempfile::{TempDir, tempdir};
use test_support::fs as test_fs;

fn workspace(context: &str) -> Result<TempDir> {
    let temp = tempdir().with_context(|| format!("create workspace for {context}"))?;
    test_fs::copy("tests/data/minimal.yml", temp.path().join("Netsukefile"))
        .with_context(|| format!("write manifest for {context}"))?;
    Ok(temp)
}

fn isolated_netsuke_command(current_dir: &std::path::Path) -> assert_cmd::Command {
    let mut command = cargo_bin_cmd!("netsuke");
    command.current_dir(current_dir).env_clear();
    command
}

#[test]
fn no_discovered_config_allows_the_graph_workflow() -> Result<()> {
    let temp = workspace("no discovered configuration")?;
    let output = isolated_netsuke_command(temp.path())
        .arg("graph")
        .output()
        .context("run graph without configuration")?;

    ensure!(output.status.success(), "graph should use defaults");
    Ok(())
}

#[test]
fn malformed_discovered_config_fails_the_binary_workflow() -> Result<()> {
    let temp = workspace("malformed discovered configuration")?;
    test_fs::write(temp.path().join(".netsuke.toml"), "emoji = \"always\n")
        .context("write malformed discovered config")?;

    let output = isolated_netsuke_command(temp.path())
        .arg("graph")
        .output()
        .context("run graph with malformed configuration")?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    ensure!(!output.status.success(), "malformed config must fail");
    ensure!(
        stderr.contains("configuration load failed")
            && stderr.contains("operation=\"diag_mode_resolution\"")
            && stderr.contains("error_category=\"io\""),
        "configuration failure should retain bounded operational context: {stderr}",
    );
    ensure!(
        !stderr.contains(".netsuke.toml"),
        "configuration failure must not expose the discovered file name: {stderr}",
    );
    Ok(())
}
