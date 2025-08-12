//! Helpers for environment manipulation in process tests.
//!
//! Provides fixtures and utilities for managing `PATH` and writing minimal
//! manifests.

use mockable::MockEnv;
use rstest::fixture;
use std::io::{self, Write};

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
