//! Helpers for environment manipulation in process tests.
//!
//! Provides fixtures and utilities for managing `PATH` and writing minimal
//! manifests.

use mockable::{DefaultEnv, Env, MockEnv};
use rstest::fixture;
use std::ffi::{OsStr, OsString};
use std::io::{self, Write};
use std::path::Path;

use crate::support::env_lock::EnvLock;
use crate::support::path_guard::PathGuard;

/// Alias for the real process environment.
#[allow(dead_code, reason = "re-exported for tests")]
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
#[fixture]
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
#[allow(dead_code, reason = "used in Cucumber tests")]
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
#[allow(dead_code, reason = "used in runner tests")]
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
