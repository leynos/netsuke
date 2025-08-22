//! Guard for temporarily modifying environment variables in tests.
//!
//! `std::env::set_var` and `remove_var` are `unsafe` in Rust 2024 because they
//! mutate process-global state. Acquire an [`EnvLock`](crate::env_lock::EnvLock)
//! before calling the provided constructors to serialise mutations across
//! threads. The guard restores the previous value on drop.
//!
//! # Examples
//!
//! ```rust,ignore
//! use test_support::env_lock::EnvLock;
//! use test_support::env_var_guard::EnvVarGuard;
//!
//! let _lock = EnvLock::acquire();
//! let _guard = EnvVarGuard::set("FOO", "bar");
//! assert_eq!(std::env::var("FOO").unwrap(), "bar");
//! // `_guard` is dropped here and restores the previous value.
//! ```
use std::{borrow::Cow, ffi::OsString};

/// RAII guard that resets an environment variable to its previous value on drop.
#[derive(Debug)]
pub struct EnvVarGuard {
    name: Cow<'static, str>,
    prev: Option<OsString>,
}

impl EnvVarGuard {
    /// Set `name` to `val`, returning a guard that restores the prior value.
    ///
    /// # Safety
    ///
    /// Mutating process-global state is `unsafe` in Rust 2024. Callers must hold
    /// an [`EnvLock`](crate::env_lock::EnvLock) to serialise mutations.
    #[must_use]
    pub fn set(name: impl Into<Cow<'static, str>>, val: &str) -> Self {
        let name = name.into();
        let prev = std::env::var_os(&*name);
        // SAFETY: `EnvLock` serialises mutations of the process environment.
        unsafe { std::env::set_var(&*name, val) };
        Self { name, prev }
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
        Self { name, prev }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        // SAFETY: `EnvLock` serialises mutations while the prior value is
        // restored.
        unsafe {
            if let Some(ref v) = self.prev {
                std::env::set_var(&*self.name, v);
            } else {
                std::env::remove_var(&*self.name);
            }
        }
    }
}
