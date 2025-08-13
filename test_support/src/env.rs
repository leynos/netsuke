//! Helpers for environment manipulation in process tests.
//!
//! Provides fixtures and utilities for managing `PATH` and writing minimal
//! manifests.

use mockable::{DefaultEnv, Env, MockEnv};
use std::ffi::{OsStr, OsString};
use std::io::{self, Write};
use std::path::Path;

use crate::{env_lock::EnvLock, path_guard::PathGuard};

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
    let new_path = std::env::join_paths(&paths).expect("Failed to join PATH entries");
    let _lock = EnvLock::acquire();
    // SAFETY: `EnvLock` serialises mutations and the guard restores on drop.
    unsafe { env.set_var("PATH", &new_path) };
    PathGuard::new(original_os)
}

const NINJA_ENV: &str = "NETSUKE_NINJA";

/// Guard that restores `NINJA_ENV` to its previous value on drop.
#[derive(Debug)]
pub struct NinjaEnvGuard {
    original: Option<OsString>,
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
/// use std::path::Path;
/// use test_support::env::{SystemEnv, override_ninja_env};
///
/// let env = SystemEnv::new();
/// let guard = override_ninja_env(&env, Path::new("/tmp/ninja"));
/// assert_eq!(std::env::var("NETSUKE_NINJA").unwrap(), "/tmp/ninja");
/// drop(guard);
/// assert!(std::env::var("NETSUKE_NINJA").is_err());
/// ```
pub fn override_ninja_env(env: &impl EnvMut, path: &Path) -> NinjaEnvGuard {
    let original = env.raw(NINJA_ENV).ok().map(OsString::from);
    let _lock = EnvLock::acquire();
    // SAFETY: `EnvLock` serialises the mutation and the guard restores on drop.
    unsafe { env.set_var(NINJA_ENV, path.as_os_str()) };
    NinjaEnvGuard { original }
}

impl Drop for NinjaEnvGuard {
    fn drop(&mut self) {
        let _lock = EnvLock::acquire();
        // SAFETY: `EnvLock` ensures exclusive access while the variable is reset.
        unsafe {
            if let Some(val) = self.original.take() {
                std::env::set_var(NINJA_ENV, val);
            } else {
                std::env::remove_var(NINJA_ENV);
            }
        }
    }
}
