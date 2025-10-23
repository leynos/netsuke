//! Helpers for environment manipulation in process tests.
//!
//! Provides fixtures and utilities for managing `PATH` and writing minimal
//! manifests.

use mockable::{DefaultEnv, Env, MockEnv};
use ninja_env::NINJA_ENV;
use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    io::{self, Write},
    path::Path,
};

use crate::{env_guard::EnvGuard, env_lock::EnvLock, path_guard::PathGuard};

/// Alias for the real process environment.
pub type SystemEnv = DefaultEnv;

/// Environment trait with mutation capabilities.
pub trait EnvMut: Env {
    /// Set `key` to `value` within the environment.
    ///
    /// # Safety
    ///
    /// Mutating global state is `unsafe` in Rust 2024. Callers must ensure the
    /// operation is serialised and rolled back appropriately.
    unsafe fn set_var(&self, key: &str, value: &OsStr);
}

impl EnvMut for DefaultEnv {
    unsafe fn set_var(&self, key: &str, value: &OsStr) {
        unsafe { std::env::set_var(key, value) };
    }
}

impl EnvMut for MockEnv {
    unsafe fn set_var(&self, key: &str, value: &OsStr) {
        unsafe { std::env::set_var(key, value) };
    }
}

/// Fixture: capture the original `PATH` via a mocked environment.
///
/// Returns a `MockEnv` that yields the current `PATH` when queried. Tests can
/// modify the real environment while the mock continues to expose the initial
/// value.
pub fn mocked_path_env() -> MockEnv {
    let original = std::env::var("PATH").unwrap_or_default();
    let mut env = MockEnv::new();
    env.expect_raw()
        .withf(|k| k == "PATH")
        .returning(move |_| Ok(original.clone()));
    env
}

/// Set an environment variable, returning its previous value.
///
/// The mutation is `unsafe` in Rust 2024 as it alters process state. The
/// unsafety is scoped by acquiring [`EnvLock`].
pub fn set_var(key: &str, value: &OsStr) -> Option<OsString> {
    let _lock = EnvLock::acquire();
    let previous = std::env::var_os(key);
    // SAFETY: `EnvLock` serialises mutations.
    unsafe { std::env::set_var(key, value) };
    previous
}

/// Remove an environment variable, returning its previous value.
///
/// The mutation is `unsafe` in Rust 2024 as it alters process state. The
/// unsafety is scoped by acquiring [`EnvLock`].
pub fn remove_var(key: &str) -> Option<OsString> {
    let _lock = EnvLock::acquire();
    let previous = std::env::var_os(key);
    // SAFETY: `EnvLock` serialises mutations.
    unsafe { std::env::remove_var(key) };
    previous
}

/// Restore multiple environment variables under a single lock.
///
/// Each `key` is reset to its corresponding prior value or removed when
/// `None`. Mutating process-wide state is `unsafe`; [`EnvLock`] serialises the
/// operations to keep tests deterministic.
///
/// # Examples
///
/// ```
/// use std::{collections::HashMap, ffi::OsStr};
/// use test_support::env::{restore_many, set_var};
///
/// let mut snapshot = HashMap::new();
/// snapshot.insert("HELLO".into(), set_var("HELLO", OsStr::new("world")));
/// restore_many(snapshot);
/// assert!(std::env::var("HELLO").is_err());
/// ```
pub fn restore_many(vars: HashMap<String, Option<OsString>>) {
    if vars.is_empty() {
        return;
    }
    let _lock = EnvLock::acquire();
    for (key, val) in vars {
        if let Some(v) = val {
            // SAFETY: `EnvLock` serialises mutations for all variables at once.
            unsafe { std::env::set_var(key, v) };
        } else {
            // SAFETY: `EnvLock` serialises mutations for all variables at once.
            unsafe { std::env::remove_var(key) };
        }
    }
}

/// Guard that restores an environment variable to its prior value on drop.
#[derive(Debug)]
pub struct VarGuard {
    inner: EnvGuard,
}

impl VarGuard {
    /// Set `key` to `value`, returning a guard that resets it on drop.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::ffi::OsStr;
    /// use test_support::env::VarGuard;
    ///
    /// let _guard = VarGuard::set("HELLO", OsStr::new("world"));
    /// assert_eq!(std::env::var("HELLO").expect("HELLO"), "world");
    /// ```
    pub fn set(key: &str, value: &OsStr) -> Self {
        let previous = set_var(key, value);
        Self {
            inner: EnvGuard::new(key.to_string(), previous),
        }
    }

    /// Remove `key`, returning a guard that restores the prior value.
    pub fn unset(key: &str) -> Self {
        let previous = remove_var(key);
        Self {
            inner: EnvGuard::new(key.to_string(), previous),
        }
    }

    /// Access the captured original value.
    pub fn original(self) -> Option<OsString> {
        self.inner.into_original()
    }
}

/// Write a minimal manifest to `file`.
///
/// The manifest declares a single `hello` target that prints a greeting.
pub fn write_manifest(file: &mut impl Write) -> io::Result<()> {
    writeln!(
        file,
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: hello\n",
            "    recipe:\n",
            "      kind: command\n",
            "      command: \"echo hi\"\n"
        ),
    )
}

/// Prepend `dir` to the real `PATH`, returning a guard that restores it.
///
/// Mutating `PATH` is `unsafe` in Rust 2024 because it alters process globals.
/// `EnvLock` serialises access and `PathGuard` rolls back the change, keeping
/// the unsafety scoped to a single test.
pub fn prepend_dir_to_path(env: &impl EnvMut, dir: &Path) -> PathGuard {
    let original = env.raw("PATH").ok();
    let original_os = original.clone().map(OsString::from);
    let mut paths: Vec<_> = original_os
        .as_ref()
        .map(|os| std::env::split_paths(os).collect())
        .unwrap_or_default();
    paths.insert(0, dir.to_path_buf());
    let new_path = match std::env::join_paths(&paths) {
        Ok(joined) => joined,
        Err(err) => panic!("failed to join PATH entries: {err}"),
    };
    let _lock = EnvLock::acquire();
    // SAFETY: `EnvLock` serialises mutations and the guard restores on drop.
    unsafe { env.set_var("PATH", &new_path) };
    PathGuard::new(original_os)
}

/// Guard that restores `NINJA_ENV` to its previous value on drop.
#[derive(Debug)]
pub struct NinjaEnvGuard {
    inner: EnvGuard,
}

/// Override the `NINJA_ENV` variable with `path`, returning a guard that resets it.
///
/// In Rust 2024 `std::env::set_var` is `unsafe` because it mutates process-global
/// state. `EnvLock` serialises the mutation and the guard restores the prior
/// value, confining the unsafety to the scope of the guard.
///
/// # Examples
///
/// ```
/// use ninja_env::NINJA_ENV;
/// use test_support::env::{SystemEnv, override_ninja_env};
///
/// let env = SystemEnv::new();
/// let path = std::env::temp_dir().join("ninja");
/// let guard = override_ninja_env(&env, path.as_path());
/// assert_eq!(
///     std::env::var(NINJA_ENV).expect("NINJA_ENV"),
///     path.to_string_lossy()
/// );
/// drop(guard);
/// assert!(std::env::var(NINJA_ENV).is_err());
/// ```
pub fn override_ninja_env(env: &impl EnvMut, path: &Path) -> NinjaEnvGuard {
    let _lock = EnvLock::acquire();
    let original = env.raw(NINJA_ENV).ok().map(OsString::from);
    // SAFETY: `EnvLock` serialises the mutation and the guard restores on drop.
    unsafe { env.set_var(NINJA_ENV, path.as_os_str()) };
    NinjaEnvGuard {
        inner: EnvGuard::new(NINJA_ENV, original),
    }
}

impl NinjaEnvGuard {
    /// Access the captured original value.
    pub fn original(self) -> Option<OsString> {
        self.inner.into_original()
    }
}
