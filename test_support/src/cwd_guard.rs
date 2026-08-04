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
    //! Unit tests for the working-directory guard.

    use super::*;
    use crate::env_lock::EnvLock;
    use rstest::{fixture, rstest};
    use std::io;

    #[fixture]
    fn env_lock() -> EnvLock {
        EnvLock::acquire()
    }

    /// The environment lock paired with the directory captured under it.
    type LockedOriginalDir = (EnvLock, io::Result<std::path::PathBuf>);

    /// Capture the directory that is current before a test mutates it.
    ///
    /// Fixtures arrange state rather than assert, so this propagates the
    /// `current_dir` failure instead of panicking; each test body unwraps it.
    ///
    /// The lock is returned rather than merely taken as a parameter: a
    /// by-value parameter is dropped when this fixture returns, which would
    /// release the lock before the test body runs and leave the body's
    /// `set_current_dir` racing other tests. Handing the guard back keeps the
    /// process-wide lock held until the test body ends.
    #[fixture]
    fn original_dir(env_lock: EnvLock) -> LockedOriginalDir {
        let captured = std::env::current_dir();
        (env_lock, captured)
    }

    #[rstest]
    #[case(CwdGuard::acquire)]
    #[case(CwdGuard::new)]
    fn constructor_captures_current_directory(
        original_dir: LockedOriginalDir,
        #[case] ctor: fn() -> io::Result<CwdGuard>,
    ) {
        let (_env_lock, captured) = original_dir;
        let expected = captured.expect("current_dir");
        let guard = ctor().expect("CwdGuard constructor");
        assert_eq!(
            guard.0, expected,
            "guard should capture the directory that was current at acquire time"
        );
    }

    #[rstest]
    fn drop_restores_original_directory(original_dir: LockedOriginalDir) {
        let (_env_lock, captured) = original_dir;
        let original_dir = captured.expect("current_dir");
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
