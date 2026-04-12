//! Restore the process working directory after tests mutate it.
//!
//! Provides a RAII guard that captures the current working directory and
//! restores it on drop so tests do not leak CWD changes into other cases.

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Guard that restores the original current working directory when dropped.
#[derive(Debug)]
pub struct CwdGuard(PathBuf);

impl CwdGuard {
    /// Capture the current working directory for later restoration.
    pub fn acquire() -> Result<Self> {
        Ok(Self(
            std::env::current_dir().context("capture current working directory")?,
        ))
    }

    /// Alias for [`CwdGuard::acquire`] to support existing test call sites.
    pub fn new() -> Result<Self> {
        Self::acquire()
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        drop(std::env::set_current_dir(&self.0));
    }
}
