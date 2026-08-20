//! End-to-end configuration discovery failure coverage.
//!
//! These tests run the real binary in a child process with a closed
//! environment, proving that missing configuration still permits the normal
//! workflow while a malformed discovered file fails before manifest handling.

use anyhow::{Context, Result, ensure};
use assert_cmd::cargo::cargo_bin_cmd;
use camino::{Utf8Path, Utf8PathBuf};
use tempfile::{TempDir, tempdir};
use test_support::fs as test_fs;

fn workspace(context: &str) -> Result<TempDir> {
    let temp = tempdir().with_context(|| format!("create workspace for {context}"))?;
    test_fs::copy("tests/data/minimal.yml", temp.path().join("Netsukefile"))
        .with_context(|| format!("write manifest for {context}"))?;
    Ok(temp)
}

fn isolated_netsuke_command(current_dir: &Utf8Path) -> assert_cmd::Command {
    let mut command = cargo_bin_cmd!("netsuke");
    command.current_dir(current_dir.as_std_path()).env_clear();
    command
}

fn utf8_workspace_path(temp: &TempDir) -> Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("workspace path {} is not UTF-8", path.display()))
}

#[test]
fn no_discovered_config_allows_the_graph_workflow() -> Result<()> {
    let temp = workspace("no discovered configuration")?;
    let workspace_path = utf8_workspace_path(&temp)?;
    let output = isolated_netsuke_command(&workspace_path)
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

    let workspace_path = utf8_workspace_path(&temp)?;
    let output = isolated_netsuke_command(&workspace_path)
        .arg("graph")
        .output()
        .context("run graph with malformed configuration")?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    ensure!(!output.status.success(), "malformed config must fail");
    ensure!(
        stderr.contains(".netsuke.toml"),
        "configuration failure should identify the discovered file: {stderr}",
    );
    Ok(())
}
