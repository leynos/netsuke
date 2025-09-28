//! Guard for temporarily modifying environment variables in tests.
//!
//! `std::env::set_var` and `remove_var` are `unsafe` in Rust 2024 because they
//! mutate process-global state. Callers **must hold** an
//! [`EnvLock`](crate::env_lock::EnvLock) for the entire lifetime of any
//! [`EnvVarGuard`] to serialise mutations across threads. The guard uses RAII to
//! restore the previous value when it is dropped.
//!
//! # Examples
//!
//! ```rust,ignore
//! use test_support::{env_lock::EnvLock, EnvVarGuard};
//!
//! let _lock = EnvLock::acquire();
//! let _guard = EnvVarGuard::set("FOO", "bar");
//! assert_eq!(std::env::var("FOO").unwrap(), "bar");
//! // The guard's `Drop` restores the prior state when it goes out of scope.
//! ```
use std::{
    borrow::Cow,
    ffi::{OsStr, OsString},
};

use crate::env_guard::EnvGuard;

/// RAII guard that resets an environment variable to its previous value on drop.
#[derive(Debug)]
pub struct EnvVarGuard {
    inner: EnvGuard,
}

impl EnvVarGuard {
    /// Set `name` to `val`, returning a guard that restores the prior value.
    ///
    /// # Safety
    ///
    /// Mutating process-global state is `unsafe` in Rust 2024. Callers must hold
    /// an [`EnvLock`](crate::env_lock::EnvLock) to serialise mutations.
    #[must_use]
    pub fn set(name: impl Into<Cow<'static, str>>, val: impl AsRef<OsStr>) -> Self {
        let name = name.into();
        let prev = std::env::var_os(&*name);
        // SAFETY: `EnvLock` serialises mutations of the process environment.
        unsafe { std::env::set_var(&*name, val.as_ref()) };
        Self {
            inner: EnvGuard::new_unlocked(name, prev),
        }
    }

    /// Remove `name`, returning a guard that restores the prior value.
    ///
    /// # Safety
    ///
    /// Callers must hold an [`EnvLock`](crate::env_lock::EnvLock) to serialise
    /// mutations of the process environment.
    #[must_use]
    pub fn remove(name: impl Into<Cow<'static, str>>) -> Self {
        let name = name.into();
        let prev = std::env::var_os(&*name);
        // SAFETY: `EnvLock` serialises mutations of the process environment.
        unsafe { std::env::remove_var(&*name) };
        Self {
            inner: EnvGuard::new_unlocked(name, prev),
        }
    }

    /// Access the captured original value. Useful when manual restoration is needed.
    pub fn original(self) -> Option<OsString> {
        self.inner.into_original()
    }
}
