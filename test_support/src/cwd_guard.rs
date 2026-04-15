//! Restore the process working directory after tests mutate it.
//!
//! Provides a RAII guard that captures the current working directory and
//! restores it on drop so tests do not leak CWD changes into other cases.

use std::path::PathBuf;

/// Guard that restores the original current working directory when dropped.
#[derive(Debug)]
pub struct CwdGuard(PathBuf);

impl CwdGuard {
    /// Capture the current working directory for later restoration.
    pub fn acquire() -> std::io::Result<Self> {
        Ok(Self(std::env::current_dir()?))
    }

    /// Alias for [`CwdGuard::acquire`] to support existing test call sites.
    pub fn new() -> std::io::Result<Self> {
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
    use rstest::{fixture, rstest};
    use std::io;

    #[fixture]
    fn env_lock() -> EnvLock {
        EnvLock::acquire()
    }

    #[fixture]
    fn original_dir(_env_lock: EnvLock) -> std::path::PathBuf {
        std::env::current_dir().expect("current_dir")
    }

    #[rstest]
    #[case(CwdGuard::acquire)]
    #[case(CwdGuard::new)]
    fn constructor_captures_current_directory(
        original_dir: std::path::PathBuf,
        #[case] ctor: fn() -> io::Result<CwdGuard>,
    ) {
        let guard = ctor().expect("CwdGuard constructor");
        assert_eq!(
            guard.0, original_dir,
            "guard should capture the directory that was current at acquire time"
        );
    }

    #[rstest]
    fn drop_restores_original_directory(original_dir: std::path::PathBuf) {
        let temp = tempfile::tempdir().expect("tempdir");

        {
            let _guard = CwdGuard::acquire().expect("CwdGuard::acquire");
            std::env::set_current_dir(temp.path()).expect("chdir to temp");
            assert_ne!(
                std::env::current_dir().expect("current_dir"),
                original_dir,
                "CWD should be temp dir inside the guard scope"
            );
        }

        assert_eq!(
            std::env::current_dir().expect("current_dir"),
            original_dir,
            "CWD should be restored after guard is dropped"
        );
    }
}
