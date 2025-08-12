//! Restore `PATH` after tests mutate it.
//!
//! Provides a guard that resets the environment variable on drop so tests do
//! not pollute global state.

use std::ffi::OsString;

use super::env_lock::EnvLock;

/// Guard that restores `PATH` to its original value when dropped.
///
/// This uses RAII to ensure the environment is reset even if a test panics.
#[allow(dead_code, reason = "only some tests mutate PATH")]
#[derive(Debug)]
pub struct PathGuard {
    original_path: Option<OsString>,
}

impl PathGuard {
    #[allow(dead_code, reason = "only some tests mutate PATH")]
    /// Create a guard capturing the current `PATH`.
    pub fn new(original: OsString) -> Self {
        Self {
            original_path: Some(original),
        }
    }
}

impl Drop for PathGuard {
    fn drop(&mut self) {
        let _lock = EnvLock::acquire();
        if let Some(path) = self.original_path.take() {
            // Nightly marks `set_var` unsafe; restoring `PATH` cleans up global state.
            unsafe { std::env::set_var("PATH", path) };
        }
    }
}
