//! Windows fake-Ninja fixtures that preserve the shared tool-test contract.

use super::ToolName;
use anyhow::{Context, Result};
use std::path::PathBuf;
use tempfile::TempDir;

use crate::exec::write_exec_with_content;

/// Create a Windows fake Ninja that validates a tool invocation and build file.
///
/// The batch file accepts a command line only when it contains both
/// `-t <expected_tool>` and `-f <existing build file>`, mirroring the Unix
/// fixture without extending the Windows contract to optional `-j` or `-C`
/// validation.
///
/// # Errors
///
/// Returns an error if the temporary directory or batch executable cannot be
/// created.
pub fn fake_ninja_expect_tool(expected_tool: ToolName) -> Result<(TempDir, PathBuf)> {
    let content = format!(
        concat!(
            "@echo off\r\n",
            "setlocal EnableDelayedExpansion\r\n",
            "set \"expected={expected}\"\r\n",
            "set \"found_tool=0\"\r\n",
            "set \"found_file=0\"\r\n",
            "set \"previous=\"\r\n",
            ":argument\r\n",
            "if \"%~1\"==\"\" goto validate\r\n",
            "if /I \"!previous!\"==\"-t\" if /I \"%~1\"==\"!expected!\" set \"found_tool=1\"\r\n",
            "if /I \"!previous!\"==\"-f\" if exist \"%~1\" set \"found_file=1\"\r\n",
            "set \"previous=%~1\"\r\n",
            "shift\r\n",
            "goto argument\r\n",
            ":validate\r\n",
            "if \"!found_tool!\"==\"0\" exit /B 1\r\n",
            "if \"!found_file!\"==\"0\" exit /B 1\r\n",
            "exit /B 0\r\n",
        ),
        expected = expected_tool.as_str(),
    );
    let dir = TempDir::new().context("fake_ninja_expect_tool: create temporary directory")?;
    let path = write_exec_with_content(dir.path(), "ninja.cmd", &content)
        .context("fake_ninja_expect_tool: write batch executable")?;
    Ok((dir, path))
}
