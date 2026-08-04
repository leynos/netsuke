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
    ///
    /// # Errors
    ///
    /// Returns an error if the current directory cannot be read.
    pub fn acquire() -> std::io::Result<Self> {
        Ok(Self(std::env::current_dir()?))
    }

    /// Alias for [`CwdGuard::acquire`] to support existing test call sites.
    ///
    /// # Errors
    ///
    /// Returns an error if the current directory cannot be read.
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
    #[fixture]
    fn env_lock() -> EnvLock {
        EnvLock::acquire()
    }

    #[fixture]
    fn original_dir(env_lock: EnvLock) -> io::Result<(EnvLock, std::path::PathBuf)> {
        Ok((env_lock, std::env::current_dir()?))
    }

    #[rstest]
    #[case(CwdGuard::acquire)]
    #[case(CwdGuard::new)]
    fn constructor_captures_current_directory(
        #[from(original_dir)] original_dir_result: io::Result<(EnvLock, std::path::PathBuf)>,
        #[case] ctor: fn() -> io::Result<CwdGuard>,
    ) -> anyhow::Result<()> {
        let (_lock, original_dir) = original_dir_result?;
        let guard = ctor()?;
        anyhow::ensure!(
            guard.0 == original_dir,
            "guard should capture the directory that was current at acquire time"
        );
        Ok(())
    }

    #[rstest]
    fn drop_restores_original_directory(
        #[from(original_dir)] original_dir_result: io::Result<(EnvLock, std::path::PathBuf)>,
    ) -> anyhow::Result<()> {
        let (_lock, original_dir) = original_dir_result?;
        let temp = tempfile::tempdir()?;

        {
            let _guard = CwdGuard::acquire()?;
            std::env::set_current_dir(temp.path())?;
            anyhow::ensure!(
                std::env::current_dir()? != original_dir,
                "CWD should be temp dir inside the guard scope"
            );
        }

        anyhow::ensure!(
            std::env::current_dir()? == original_dir,
            "CWD should be restored after guard is dropped"
        );
        Ok(())
    }
}
