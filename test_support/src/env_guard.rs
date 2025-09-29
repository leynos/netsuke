//! Generic guard for restoring environment variables.

use std::{
    borrow::Cow,
    ffi::{OsStr, OsString},
};

use crate::env_lock::EnvLock;

/// Environment abstraction for applying environment mutations.
///
/// The trait is intentionally minimal so tests can provide mocks.
pub trait Environment {
    /// Set `key` to `value` within the environment.
    ///
    /// # Safety
    ///
    /// Mutating process-global state is `unsafe` in Rust 2024. Callers must
    /// ensure mutations are serialised and restored.
    unsafe fn set_var(&mut self, key: &str, value: &OsStr);

    /// Remove `key` from the environment.
    ///
    /// # Safety
    ///
    /// Mutating process-global state is `unsafe` in Rust 2024. Callers must
    /// ensure mutations are serialised and restored.
    unsafe fn remove_var(&mut self, key: &str);
}

/// Concrete [`Environment`] backed by [`std::env`].
#[derive(Debug, Default)]
pub struct StdEnv;

impl Environment for StdEnv {
    unsafe fn set_var(&mut self, key: &str, value: &OsStr) {
        unsafe { std::env::set_var(key, value) };
    }

    unsafe fn remove_var(&mut self, key: &str) {
        unsafe { std::env::remove_var(key) };
    }
}

/// RAII guard that restores an environment variable to its prior state on drop.
///
/// The guard captures the previous value of `key` and reinstates it when dropped,
/// ensuring tests leave global process state untouched even if they panic.
#[derive(Debug)]
pub struct EnvGuard<E: Environment = StdEnv> {
    key: Cow<'static, str>,
    original: Option<OsString>,
    env: E,
    lock_on_drop: bool,
}

impl EnvGuard {
    /// Create a guard for `key` using [`StdEnv`].
    pub fn new(key: impl Into<Cow<'static, str>>, original: Option<OsString>) -> Self {
        Self::with_env_and_lock(key, original, StdEnv::default(), true)
    }

    /// Create a guard that skips locking on drop.
    pub fn new_unlocked(key: impl Into<Cow<'static, str>>, original: Option<OsString>) -> Self {
        Self::with_env_and_lock(key, original, StdEnv::default(), false)
    }
}

impl<E: Environment> EnvGuard<E> {
    /// Create a guard for `key` using a custom environment implementation.
    pub fn with_env(key: impl Into<Cow<'static, str>>, original: Option<OsString>, env: E) -> Self {
        Self::with_env_and_lock(key, original, env, true)
    }

    /// Create a guard for `key` with an explicit locking strategy.
    pub fn with_env_and_lock(
        key: impl Into<Cow<'static, str>>,
        original: Option<OsString>,
        env: E,
        lock_on_drop: bool,
    ) -> Self {
        Self {
            key: key.into(),
            original,
            env,
            lock_on_drop,
        }
    }

    /// Access the underlying environment implementation.
    pub fn env_mut(&mut self) -> &mut E {
        &mut self.env
    }

    /// Consume the guard returning the captured original value.
    pub fn into_original(self) -> Option<OsString> {
        self.original.clone()
    }

    fn restore(&mut self) {
        match self.original.take() {
            Some(value) => unsafe { self.env.set_var(&self.key, &value) },
            None => unsafe { self.env.remove_var(&self.key) },
        }
    }
}

impl<E: Environment> Drop for EnvGuard<E> {
    fn drop(&mut self) {
        if self.lock_on_drop {
            let _lock = EnvLock::acquire();
            self.restore();
        } else {
            self.restore();
        }
    }
}
