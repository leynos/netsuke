//! Tests for explicit configuration-path selection.
//!
//! These tests keep explicit relative selectors anchored to the process CWD
//! instead of rebasing them beneath the CLI directory.

use super::*;
use crate::cli::test_support::TestEnv;
use anyhow::{Context, Result, ensure};
use tempfile::tempdir;

/// An explicit relative configuration file does not use the CLI directory.
#[test]
fn explicit_relative_config_does_not_use_cli_directory() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let unique_dir_name = temp
        .path()
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .context("temporary directory has a UTF-8 name")?;
    let config_name = format!("{unique_dir_name}-relative-config.toml");
    let config_path = temp.path().join(&config_name);
    test_support::fs::write(&config_path, "emoji = \"always\"\n")
        .context("write explicit config")?;
    let cli = Cli {
        config: Some(config_name.into()),
        directory: Some(temp.path().to_path_buf()),
        ..Cli::default()
    };

    let discovered = discover_file_layers(&cli, &TestEnv::default());
    let error = discovered
        .first_error()
        .context("relative explicit config must not load from the CLI directory")?;

    ensure!(
        error
            .to_string()
            .contains("explicit configuration file not found"),
        "expected missing explicit config error, got {error}"
    );
    Ok(())
}
