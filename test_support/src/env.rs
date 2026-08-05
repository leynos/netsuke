//! Helpers for composing child-process environments in tests.
//!
//! Provides fixtures and utilities for managing `PATH` and writing minimal
//! manifests.

use anyhow::{Context, Result};
use std::{
    ffi::{OsStr, OsString},
    io::{self, Write},
    path::Path,
};

/// Write a minimal manifest to `file`.
///
/// The manifest declares a single `hello` target that prints a greeting.
///
/// # Errors
///
/// Returns an error if the manifest cannot be written.
pub fn write_manifest(file: &mut impl Write) -> io::Result<()> {
    writeln!(
        file,
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: hello\n",
            "    command: \"echo hi\"\n"
        ),
    )
}

/// Compose a `PATH` value with `dir` prepended to the supplied prior value.
///
/// # Errors
///
/// Returns an error if the path entries cannot be joined for the host platform.
pub fn prepend_path_value(original: Option<&OsStr>, dir: &Path) -> Result<OsString> {
    let mut paths = vec![dir.to_path_buf()];
    if let Some(value) = original.filter(|value| !value.is_empty()) {
        paths.extend(std::env::split_paths(value));
    }
    std::env::join_paths(paths)
        .with_context(|| format!("failed to prepend {} to PATH", dir.display()))
}
