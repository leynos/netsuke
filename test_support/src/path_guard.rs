//! Restore `PATH` after tests mutate it.
//!
//! Provides a guard that resets the environment variable on drop so tests do
//! not pollute global state.

use std::ffi::{OsStr, OsString};

use crate::env_guard::{EnvGuard, Environment, StdEnv};
use crate::env_lock::EnvLock;

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
    /// Create a guard that restores `PATH` to `original` when dropped.
    ///
    /// Callers supply the value to reinstate; use [`PathGuard::capture`] to
    /// snapshot the current `PATH` automatically.
    pub fn new(original: Option<OsString>) -> Self {
        Self {
            inner: EnvGuard::with_env_and_lock("PATH", original, StdEnv::default(), true),
        }
    }

    /// Capture the current `PATH` from the real environment.
    ///
    /// Zero-argument convenience for callers using the real environment: the
    /// guard snapshots `PATH` at construction and restores that snapshot when
    /// dropped, so tests need not fetch and pass the value manually.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use test_support::PathGuard;
    ///
    /// let guard = PathGuard::capture();
    /// // Mutate PATH as the test requires; the original value is restored
    /// // when `guard` drops.
    /// drop(guard);
    /// ```
    #[must_use]
    pub fn capture() -> Self {
        Self::new(std::env::var_os("PATH"))
    }
}

impl<E: Env> PathGuard<E> {
    /// Create a guard that uses `env` to restore `PATH`.
    pub fn with_env(original: Option<OsString>, env: E) -> Self {
        Self {
            inner: EnvGuard::with_env_and_lock("PATH", original, env, true),
        }
    }

    /// Access the underlying environment.
    pub fn env_mut(&mut self) -> &mut E {
        self.inner.env_mut()
    }

    /// Set `PATH` to `value` through the guard's environment.
    ///
    /// Safe wrapper around the `unsafe` [`Environment::set_var`] operation:
    /// the global [`EnvLock`] serialises the mutation and the guard restores
    /// the original value on drop, so the unsafety stays encapsulated here
    /// rather than leaking `unsafe` blocks into consumer code.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::ffi::OsStr;
    /// use test_support::PathGuard;
    ///
    /// let mut guard = PathGuard::capture();
    /// guard.set_path(OsStr::new("/stub/bin"));
    /// // The captured PATH is restored when `guard` drops.
    /// ```
    pub fn set_path(&mut self, value: &OsStr) {
        let _lock = EnvLock::acquire();
        // SAFETY: `EnvLock` serialises the mutation and the guard restores
        // the captured value on drop.
        unsafe { self.inner.env_mut().set_var("PATH", value) };
    }
}
