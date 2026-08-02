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
use std::fs;
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::time::SystemTime;

use super::Sandbox;

impl Sandbox {
    /// Write a fixture file, creating its parent directory.
    ///
    /// Offered here, along with [`create_dir`](Self::create_dir) and
    /// [`write_file_with_mtime`](Self::write_file_with_mtime), so test crates
    /// can stage fixtures without reaching for `std::fs` themselves. That keeps
    /// the ambient filesystem access inside this already-sanctioned support
    /// crate instead of widening the Whitaker exclusion list.
    pub fn write_file(&self, path: &Utf8Path, contents: &str) -> Result<()> {
        self.create_parent(path)?;
        fs::write(path.as_std_path(), contents).with_context(|| format!("write {path}"))
    }

    /// Write a fixture file and backdate it to `mtime`, in seconds since the
    /// Unix epoch.
    ///
    /// Tests that need to observe a later `touch` compare against a fixed old
    /// timestamp rather than against each other, which keeps the observation
    /// free of filesystem timestamp granularity.
    pub fn write_file_with_mtime(
        &self,
        path: &Utf8Path,
        contents: &str,
        mtime: SystemTime,
    ) -> Result<()> {
        self.create_parent(path)?;
        let file = File::create(path.as_std_path()).with_context(|| format!("create {path}"))?;
        file.write_all_at(contents.as_bytes(), 0)
            .with_context(|| format!("write {path}"))?;
        file.set_modified(mtime)
            .with_context(|| format!("backdate {path}"))
    }

    /// Read a file staged in the sandbox.
    ///
    /// A missing file is an error rather than an empty string: a test asserting
    /// on recorded output would otherwise read "the command was never run" as
    /// "the command recorded nothing".
    pub fn read_file(&self, path: &Utf8Path) -> Result<String> {
        fs::read_to_string(path.as_std_path()).with_context(|| format!("read {path}"))
    }

    /// A file's modification time, in whole seconds since the Unix epoch.
    ///
    /// Whole seconds because the callers compare against a deliberately
    /// backdated stamp, not against each other.
    pub fn mtime_seconds(&self, path: &Utf8Path) -> Result<i64> {
        let modified = fs::metadata(path.as_std_path())
            .with_context(|| format!("stat {path}"))?
            .modified()
            .with_context(|| format!("read mtime of {path}"))?;
        let since_epoch = modified
            .duration_since(SystemTime::UNIX_EPOCH)
            .with_context(|| format!("{path} predates the Unix epoch"))?;
        i64::try_from(since_epoch.as_secs()).context("mtime does not fit in i64")
    }

    /// Create a directory and any missing parents.
    pub fn create_dir(&self, path: &Utf8Path) -> Result<()> {
        fs::create_dir_all(path.as_std_path()).with_context(|| format!("create {path}"))
    }

    fn create_parent(&self, path: &Utf8Path) -> Result<()> {
        match path.parent() {
            Some(parent) => self.create_dir(parent),
            None => Ok(()),
        }
    }
}
