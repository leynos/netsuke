//! Helpers for creating executable stubs in tests.
//!
//! These utilities write tiny shell/batch scripts and mark them executable so
//! tests can exercise PATH resolution without depending on real binaries.
//! Callers own the containing directory's lifetime to keep the stub on disk.
//!
//! # Examples
//!
//! ```rust
//! use tempfile::TempDir;
//! use test_support::write_exec;
//!
//! let temp = TempDir::new().expect("tempdir");
//! let path = write_exec(temp.path(), "tool").expect("stub executable");
//! assert!(path.exists());
//! ```

use crate::fs;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Write a minimal executable file named `name` inside `root`.
pub fn write_exec(root: &Path, name: &str) -> Result<PathBuf> {
    write_exec_with_content(root, name, "#!/bin/sh\n")
}

/// Write an executable script named `name` inside `root` with `content`.
///
/// This is the shared primitive behind the fake-executable helpers: it
/// creates the file, writes the script body verbatim, and marks the result
/// executable on Unix. Callers provide platform-appropriate content (for
/// example a POSIX shell script on Unix or a batch file on Windows).
///
/// # Examples
///
/// ```rust
/// use tempfile::TempDir;
/// use test_support::exec::write_exec_with_content;
///
/// let temp = TempDir::new().expect("tempdir");
/// let path = write_exec_with_content(temp.path(), "tool", "#!/bin/sh\nexit 3\n")
///     .expect("stub executable");
/// assert!(path.exists());
/// ```
pub fn write_exec_with_content(root: &Path, name: &str, content: &str) -> Result<PathBuf> {
    let path = root.join(name);
    fs::write(&path, content).with_context(|| format!("write exec stub {name}"))?;
    make_executable(&path)?;
    Ok(path)
}

/// Mark an existing file as executable by setting its Unix permission bits.
#[cfg(unix)]
pub fn make_executable(path: &Path) -> Result<()> {
    fs::set_mode(path, 0o755).context("chmod exec stub")?;
    Ok(())
}

/// No-op on non-Unix platforms, where executability is not a permission bit.
#[cfg(not(unix))]
pub fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}
