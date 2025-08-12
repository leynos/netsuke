//! Helpers for environment manipulation in process tests.
//!
//! Provides fixtures and utilities for managing `PATH` and writing minimal
//! manifests.

use mockable::{Env, MockEnv};
use rstest::fixture;
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;

use crate::support::env_lock::EnvLock;
use crate::support::path_guard::PathGuard;

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
/// `std::env::set_var` is `unsafe` in Rust 2024 because it mutates process
/// globals. `EnvLock` serialises access and `PathGuard` rolls back the change,
/// keeping the unsafety scoped to a single test.
#[allow(dead_code, reason = "used in runner tests")]
pub fn prepend_dir_to_path(env: &impl Env, dir: &Path) -> PathGuard {
    let original = env.raw("PATH").unwrap_or_default();
    let original_os: OsString = original.clone().into();
    let mut paths: Vec<_> = std::env::split_paths(&original_os).collect();
    paths.insert(0, dir.to_path_buf());
    let new_path = std::env::join_paths(paths).expect("join paths");
    let _lock = EnvLock::acquire();
    // Mockable's `Env` trait cannot mutate variables, so call directly.
    // SAFETY: `EnvLock` serialises mutations and the guard restores on drop.
    unsafe { std::env::set_var("PATH", &new_path) };
    PathGuard::new(original_os)
}
