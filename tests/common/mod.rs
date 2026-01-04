//! Shared helpers for workflow validation tests.
//!
//! Integration tests under `tests/` compile as independent crates. This module
//! is included via `mod common;` in workflow test files to share helpers while
//! keeping those modules small and avoiding duplication.

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Load a workflow file from `.github/workflows`.
pub fn workflow_contents(name: &str) -> Result<String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = root.join(".github").join("workflows").join(name);
    fs::read_to_string(&path)
        .with_context(|| format!("read workflow contents from {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::workflow_contents;

    #[test]
    fn workflow_contents_reads_release_workflow() {
        let _contents =
            workflow_contents("release.yml").expect("release workflow should be readable");
    }
}
