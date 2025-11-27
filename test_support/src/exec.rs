//! Helpers for creating executable stubs in tests.
//!
//! These utilities write tiny shell/batch scripts and mark them executable so
//! tests can exercise PATH resolution without depending on real binaries.
//! Callers own the containing directory's lifetime to keep the stub on disk.
//!
//! # Examples
//!
//! ```rust
//! use camino::Utf8Path;
//! use tempfile::TempDir;
//! use test_support::write_exec;
//!
//! let temp = TempDir::new().expect("tempdir");
//! let root = Utf8Path::from_path(temp.path()).expect("utf8 path");
//! let path = write_exec(root, "tool").expect("stub executable");
//! assert!(path.exists());
//! ```

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use std::fs;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Write a minimal executable file named `name` inside `root`.
pub fn write_exec(root: &Utf8Path, name: &str) -> Result<Utf8PathBuf> {
    let path = root.join(name);
    fs::write(path.as_std_path(), b"#!/bin/sh\n")
        .with_context(|| format!("write exec stub {name}"))?;
    make_executable(&path)?;
    Ok(path)
}

/// Mark an existing file as executable on Unix; no-op elsewhere.
pub fn make_executable(path: &Utf8Path) -> Result<()> {
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(path.as_std_path())
            .context("stat exec stub")?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path.as_std_path(), perms).context("chmod exec stub")?;
    }

    #[cfg(not(unix))]
    let _ = path;

    Ok(())
}
