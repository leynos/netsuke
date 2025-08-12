//! Restore `PATH` after tests mutate it.
//!
//! Provides a guard that resets the environment variable on drop so tests do
//! not pollute global state.

use std::ffi::{OsStr, OsString};

use super::env_lock::EnvLock;

/// Environment interface allowing `PATH` mutation.
pub trait Env {
    /// Set an environment variable.
    ///
    /// # Safety
    ///
    /// Mutating process-wide state is `unsafe` in Rust 2024.
    unsafe fn set_var(&mut self, key: &str, val: &OsStr);
}

/// Real environment implementation.
#[derive(Debug, Default)]
pub struct RealEnv;

impl Env for RealEnv {
    #[allow(unsafe_op_in_unsafe_fn, reason = "delegates to std::env")]
    unsafe fn set_var(&mut self, key: &str, val: &OsStr) {
        std::env::set_var(key, val);
    }
}

/// Guard that restores `PATH` to its original value when dropped.
///
/// This uses RAII to ensure the environment is reset even if a test panics.
#[allow(dead_code, reason = "only some tests mutate PATH")]
#[derive(Debug)]
pub struct PathGuard<E: Env = RealEnv> {
    env: E,
    original_path: Option<OsString>,
}

impl PathGuard {
    #[allow(dead_code, reason = "only some tests mutate PATH")]
    /// Create a guard capturing the current `PATH` using the real environment.
    pub fn new(original: OsString) -> Self {
        Self::with_env(original, RealEnv)
    }
}

impl<E: Env> PathGuard<E> {
    #[allow(dead_code, reason = "only some tests mutate PATH")]
    /// Create a guard for `PATH` using a provided environment.
    pub fn with_env(original: OsString, env: E) -> Self {
        Self {
            env,
            original_path: Some(original),
        }
    }

    /// Access the underlying environment for mutation during a test.
    #[allow(dead_code, reason = "used in env injection tests")]
    pub fn env_mut(&mut self) -> &mut E {
        &mut self.env
    }

    fn restore(&mut self) {
        let _lock = EnvLock::acquire();
        if let Some(path) = self.original_path.take() {
            // SAFETY: `std::env::set_var` is `unsafe` in Rust 2024 because it
            // mutates process-wide state. `EnvLock` serialises mutations and
            // this guard's RAII drop restores the prior `PATH`, mitigating the
            // unsafety.
            unsafe { self.env.set_var("PATH", &path) };
        }
    }
}

impl<E: Env> Drop for PathGuard<E> {
    fn drop(&mut self) {
        self.restore();
    }
}
