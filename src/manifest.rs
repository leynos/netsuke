//! Manifest loading helpers.
//!
//! This module provides convenience functions for parsing a static
//! `Netsukefile` into the [`crate::ast::NetsukeManifest`] structure.
//! They wrap `serde_yml` and add basic file handling.

use crate::ast::NetsukeManifest;
use anyhow::{Context, Result};
use std::{fs, path::Path};

/// Parse a YAML string into a [`NetsukeManifest`].
///
/// # Examples
///
/// ```
/// use netsuke::manifest::from_str;
///
/// let yaml = r#"
/// netsuke_version: 1.0.0
/// targets:
///   - name: a
///     command: echo hi
/// "#;
/// let manifest = from_str(yaml).expect("parse");
/// assert_eq!(manifest.targets.len(), 1);
/// ```
///
/// # Errors
///
/// Returns an error if the YAML is malformed or fails validation.
pub fn from_str(yaml: &str) -> Result<NetsukeManifest> {
    serde_yml::from_str::<NetsukeManifest>(yaml).context("YAML parse error")
}

/// Load a [`NetsukeManifest`] from the given file path.
///
/// # Examples
///
/// ```no_run
/// use netsuke::manifest::from_path;
///
/// let manifest = from_path("Netsukefile");
/// assert!(manifest.is_ok());
/// ```
///
/// # Errors
///
/// Returns an error if the file cannot be read or the YAML fails to parse.
pub fn from_path(path: impl AsRef<Path>) -> Result<NetsukeManifest> {
    let path_ref = path.as_ref();
    let data = fs::read_to_string(path_ref)
        .with_context(|| format!("Failed to read {}", path_ref.display()))?;
    from_str(&data)
}
