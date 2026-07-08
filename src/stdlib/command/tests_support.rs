//! Shared `#[cfg(test)]` helpers for the command module test suites.

use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use tempfile::tempdir;

use super::CommandConfig;
use crate::stdlib::{DEFAULT_COMMAND_MAX_OUTPUT_BYTES, DEFAULT_COMMAND_MAX_STREAM_BYTES};

/// Build a [`CommandConfig`] rooted at a fresh temporary workspace using the
/// default output and stream byte budgets. The returned [`tempfile::TempDir`]
/// guard must be kept alive for the workspace directory to persist.
pub(super) fn test_command_config() -> Result<(tempfile::TempDir, CommandConfig)> {
    let temp = tempdir().context("create command temp workspace")?;
    let path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("temp workspace should be valid UTF-8: {path:?}"))?;
    let dir =
        Dir::open_ambient_dir(&path, ambient_authority()).context("open temp workspace dir")?;
    let config = CommandConfig::new(
        DEFAULT_COMMAND_MAX_OUTPUT_BYTES,
        DEFAULT_COMMAND_MAX_STREAM_BYTES,
        Arc::new(dir),
        Some(Arc::new(path)),
    );
    Ok((temp, config))
}
