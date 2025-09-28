//! Restore `PATH` after tests mutate it.
//!
//! Provides a guard that resets the environment variable on drop so tests do
//! not pollute global state.

use std::ffi::OsString;

use crate::env_guard::{EnvGuard, Environment, StdEnv};

/// Environment abstraction for setting variables.
pub trait Env: Environment {}

impl<T: Environment> Env for T {}

/// Guard that restores `PATH` to its original value when dropped.
///
/// This uses RAII to ensure the environment is reset even if a test panics.
#[derive(Debug)]
pub struct PathGuard<E: Env = StdEnv> {
    inner: EnvGuard<E>,
}

impl PathGuard {
    /// Create a guard capturing the current `PATH` using the real environment.
    ///
    /// Returns a guard that restores the variable when dropped.
    pub fn new(original: Option<OsString>) -> Self {
        Self {
            inner: EnvGuard::with_env_and_lock("PATH", original, StdEnv::default(), true),
        }
    }
}

impl<E: Env> PathGuard<E> {
    /// Create a guard that uses `env` to restore `PATH`.
    pub fn with_env(original: OsString, env: E) -> Self {
        Self {
            inner: EnvGuard::with_env_and_lock("PATH", Some(original), env, true),
        }
    }

    /// Access the underlying environment.
    pub fn env_mut(&mut self) -> &mut E {
        self.inner.env_mut()
    }
}
