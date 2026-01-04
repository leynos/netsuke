//! Shared helpers for integration tests.
//!
//! Integration tests under `tests/` compile as independent crates. This module
//! is included via `mod common;` in individual test files to share fixtures and
//! helpers while keeping test modules small and avoiding duplication.

use anyhow::{Context, Result};
use rstest::fixture;
use std::fs;
use std::path::PathBuf;
use test_support::{
    env::{NinjaEnvGuard, SystemEnv, override_ninja_env},
    fake_ninja,
};

/// Create a temporary project with a Netsukefile from `minimal.yml`.
pub fn create_test_manifest() -> Result<(tempfile::TempDir, PathBuf)> {
    let temp = tempfile::tempdir().context("create temp dir for test manifest")?;
    let manifest_path = temp.path().join("Netsukefile");
    std::fs::copy("tests/data/minimal.yml", &manifest_path)
        .with_context(|| format!("copy minimal.yml to {}", manifest_path.display()))?;
    Ok((temp, manifest_path))
}

/// Fixture: point `NINJA_ENV` at a fake `ninja` with a configurable exit code.
///
/// Returns: (tempdir holding ninja, `NINJA_ENV` guard)
#[fixture]
pub fn ninja_with_exit_code(
    #[default(0u8)] exit_code: u8,
) -> Result<(tempfile::TempDir, PathBuf, NinjaEnvGuard)> {
    let (ninja_dir, ninja_path) = fake_ninja(exit_code)?;
    let env = SystemEnv::new();
    let guard = override_ninja_env(&env, ninja_path.as_path());
    Ok((ninja_dir, ninja_path, guard))
}

/// Load a workflow file from `.github/workflows`.
pub fn workflow_contents(name: &str) -> Result<String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = root.join(".github").join("workflows").join(name);
    fs::read_to_string(&path)
        .with_context(|| format!("read workflow contents from {}", path.display()))
}
