//! Restore `PATH` after tests mutate it.
//!
//! Provides a guard that resets the environment variable on drop so tests do
//! not pollute global state.

use std::ffi::OsString;

/// Guard that restores `PATH` to its original value when dropped.
///
/// This uses RAII to ensure the environment is reset even if a test panics.
#[derive(Debug)]
pub struct PathGuard {
    original: OsString,
}

impl PathGuard {
    /// Create a guard capturing the current `PATH`.
    pub fn new(original: OsString) -> Self {
        Self { original }
    }
}

impl Drop for PathGuard {
    fn drop(&mut self) {
        // Nightly marks `set_var` unsafe; restoring `PATH` cleans up global state.
        unsafe { std::env::set_var("PATH", &self.original) };
    }
}
