//! Staging fixtures into a [`Sandbox`].
//!
//! Split from `sandbox.rs` to keep both files within the repository's
//! file-length limit. These are `Sandbox` methods, offered so test crates can
//! write, read, and time-stamp fixtures without reaching for `std::fs`
//! themselves — which keeps the ambient filesystem access inside this
//! already-sanctioned support crate rather than widening the Whitaker
//! exclusion list.

use anyhow::{Context, Result};
use camino::Utf8Path;
use std::time::SystemTime;

use super::Sandbox;
use crate::fs;

impl Sandbox {
    /// Write a fixture file, creating its parent directory.
    ///
    /// Offered here, along with [`create_dir`](Self::create_dir) and
    /// [`write_file_with_mtime`](Self::write_file_with_mtime), so test crates
    /// can stage fixtures without reaching for `std::fs` themselves. That keeps
    /// the ambient filesystem access inside this already-sanctioned support
    /// crate instead of widening the Whitaker exclusion list.
    ///
    /// # Errors
    ///
    /// Returns an error if the staged file cannot be written.
    pub fn write_file(&self, path: &Utf8Path, contents: &str) -> Result<()> {
        self.create_parent(path)?;
        fs::write(path, contents).with_context(|| format!("write {path}"))
    }

    /// Write a fixture file and backdate it to `mtime`, in seconds since the
    /// Unix epoch.
    ///
    /// Tests that need to observe a later `touch` compare against a fixed old
    /// timestamp rather than against each other, which keeps the observation
    /// free of filesystem timestamp granularity.
    ///
    /// # Errors
    ///
    /// Returns an error if the staged file or its modification time cannot be written.
    pub fn write_file_with_mtime(
        &self,
        path: &Utf8Path,
        contents: &str,
        mtime: SystemTime,
    ) -> Result<()> {
        self.create_parent(path)?;
        fs::write_with_mtime(path, contents, mtime)
            .with_context(|| format!("write and backdate {path}"))
    }

    /// Read a file staged in the sandbox.
    ///
    /// A missing file is an error rather than an empty string: a test asserting
    /// on recorded output would otherwise read "the command was never run" as
    /// "the command recorded nothing".
    ///
    /// # Errors
    ///
    /// Returns an error if the staged file cannot be read.
    pub fn read_file(&self, path: &Utf8Path) -> Result<String> {
        fs::read_to_string(path).with_context(|| format!("read {path}"))
    }

    /// A file's modification time, in whole seconds since the Unix epoch.
    ///
    /// Whole seconds because the callers compare against a deliberately
    /// backdated stamp, not against each other.
    ///
    /// # Errors
    ///
    /// Returns an error if the staged file metadata cannot be read.
    pub fn mtime_seconds(&self, path: &Utf8Path) -> Result<i64> {
        let modified = fs::modified(path).with_context(|| format!("read mtime of {path}"))?;
        let since_epoch = modified
            .duration_since(SystemTime::UNIX_EPOCH)
            .with_context(|| format!("{path} predates the Unix epoch"))?;
        i64::try_from(since_epoch.as_secs()).context("mtime does not fit in i64")
    }

    /// Create a directory and any missing parents.
    ///
    /// # Errors
    ///
    /// Returns an error if the staged directory cannot be created.
    pub fn create_dir(&self, path: &Utf8Path) -> Result<()> {
        fs::create_dir_all(path).with_context(|| format!("create {path}"))
    }

    /// Create `path`'s parent directory, leaving a parentless path untouched.
    ///
    /// # Errors
    ///
    /// Returns an error if the parent directory cannot be created.
    fn create_parent(&self, path: &Utf8Path) -> Result<()> {
        path.parent()
            .map_or_else(|| Ok(()), |parent| self.create_dir(parent))
    }
}
