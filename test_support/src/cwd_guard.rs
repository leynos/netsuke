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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env_lock::EnvLock;

    #[test]
    fn acquire_captures_current_directory() {
        let _lock = EnvLock::acquire();
        let original = std::env::current_dir().expect("current_dir");
        let guard = CwdGuard::acquire().expect("CwdGuard::acquire");
        assert_eq!(
            guard.0, original,
            "guard should capture the directory that was current at acquire time"
        );
    }

    #[test]
    fn drop_restores_original_directory() {
        let _lock = EnvLock::acquire();
        let original = std::env::current_dir().expect("current_dir");
        let temp = tempfile::tempdir().expect("tempdir");

        {
            let _guard = CwdGuard::acquire().expect("CwdGuard::acquire");
            std::env::set_current_dir(temp.path()).expect("chdir to temp");
            assert_ne!(
                std::env::current_dir().expect("current_dir"),
                original,
                "CWD should be temp dir inside the guard scope"
            );
        }

        assert_eq!(
            std::env::current_dir().expect("current_dir"),
            original,
            "CWD should be restored after guard is dropped"
        );
    }

    #[test]
    fn new_is_alias_for_acquire() {
        let _lock = EnvLock::acquire();
        let original = std::env::current_dir().expect("current_dir");
        let guard = CwdGuard::new().expect("CwdGuard::new");
        assert_eq!(guard.0, original);
    }
}
