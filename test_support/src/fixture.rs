//! Shared test fixtures for integration-test workspaces.

use crate::fs;
use anyhow::{Context, Result};
use std::path::Path;
use tempfile::TempDir;

/// Create a temporary workspace seeded with the minimal manifest fixture.
///
/// Copies `tests/data/minimal.yml` from the crate that supplies `fixture_root`
/// into a fresh `Netsukefile` inside a new temporary directory, so integration
/// tests start from the same known manifest.
///
/// # Errors
///
/// Returns an error if the temporary directory cannot be created or the
/// fixture manifest cannot be copied, naming `context` so the failing test
/// can be identified.
pub fn setup_minimal_workspace(fixture_root: impl AsRef<Path>, context: &str) -> Result<TempDir> {
    let temp = tempfile::tempdir().with_context(|| format!("create temp dir for {context}"))?;
    let manifest = temp.path().join("Netsukefile");
    let source = fixture_root.as_ref().join("tests/data/minimal.yml");
    fs::copy(&source, &manifest)
        .with_context(|| format!("copy {} to {}", source.display(), manifest.display()))?;
    Ok(temp)
}
