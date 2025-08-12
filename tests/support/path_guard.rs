//! Restore `PATH` after tests mutate it.
//!
//! Provides a guard that resets the environment variable on drop so tests do
//! not pollute global state.

use std::ffi::OsString;

use super::env_lock::EnvLock;

/// Original `PATH` state captured by `PathGuard`.
#[allow(dead_code, reason = "only some tests mutate PATH")]
#[derive(Debug)]
enum OriginalPath {
    Unset,
    Set(OsString),
}

/// Guard that restores `PATH` to its original value when dropped.
///
/// This uses RAII to ensure the environment is reset even if a test panics.
#[allow(dead_code, reason = "only some tests mutate PATH")]
#[derive(Debug)]
pub struct PathGuard {
    original: Option<OriginalPath>,
}

impl PathGuard {
    /// Create a guard capturing the current `PATH`.
    ///
    /// Returns a guard that restores the variable when dropped.
    #[allow(dead_code, reason = "only some tests mutate PATH")]
    pub fn new(original: Option<OsString>) -> Self {
        let state = original.map_or(OriginalPath::Unset, OriginalPath::Set);
        Self {
            original: Some(state),
        }
    }
}

impl Drop for PathGuard {
    fn drop(&mut self) {
        let _lock = EnvLock::acquire();
        match self.original.take() {
            Some(OriginalPath::Set(path)) => {
                // Nightly marks `set_var` unsafe; restoring cleans up global state.
                unsafe { std::env::set_var("PATH", path) };
            }
            Some(OriginalPath::Unset) | None => unsafe { std::env::remove_var("PATH") },
        }
    }
}
