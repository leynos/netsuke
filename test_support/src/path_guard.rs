//! Restore `PATH` after tests mutate it.
//!
//! Provides a guard that resets the environment variable on drop so tests do
//! not pollute global state.

use std::ffi::{OsStr, OsString};

use crate::env_lock::EnvLock;

/// Environment abstraction for setting variables.
pub trait Env {
    /// Set `key` to `val` within the environment.
    ///
    /// # Safety
    ///
    /// Mutating process globals is `unsafe` in Rust 2024. Callers must ensure
    /// access is serialised and state is restored.
    unsafe fn set_var(&mut self, key: &str, val: &OsStr);
}

#[derive(Debug)]
pub struct StdEnv;

impl Env for StdEnv {
    unsafe fn set_var(&mut self, key: &str, val: &OsStr) {
        unsafe { std::env::set_var(key, val) };
    }
}

/// Original `PATH` state captured by `PathGuard`.
#[derive(Debug)]
enum OriginalPath {
    Unset,
    Set(OsString),
}

/// Guard that restores `PATH` to its original value when dropped.
///
/// This uses RAII to ensure the environment is reset even if a test panics.
#[derive(Debug)]
pub struct PathGuard<E: Env = StdEnv> {
    original: Option<OriginalPath>,
    env: E,
}

impl PathGuard {
    /// Create a guard capturing the current `PATH` using the real environment.
    ///
    /// Returns a guard that restores the variable when dropped.
    pub fn new(original: Option<OsString>) -> Self {
        let state = original.map_or(OriginalPath::Unset, OriginalPath::Set);
        Self {
            original: Some(state),
            env: StdEnv,
        }
    }
}

impl<E: Env> PathGuard<E> {
    /// Create a guard that uses `env` to restore `PATH`.
    pub fn with_env(original: OsString, env: E) -> Self {
        Self {
            original: Some(OriginalPath::Set(original)),
            env,
        }
    }

    /// Access the underlying environment.
    pub fn env_mut(&mut self) -> &mut E {
        &mut self.env
    }
}

impl<E: Env> Drop for PathGuard<E> {
    fn drop(&mut self) {
        let _lock = EnvLock::acquire();
        match self.original.take() {
            Some(OriginalPath::Set(path)) => {
                // Nightly marks `set_var` unsafe; restoring cleans up global state.
                unsafe { self.env.set_var("PATH", &path) };
            }
            Some(OriginalPath::Unset) | None => unsafe { std::env::remove_var("PATH") },
        }
    }
}
