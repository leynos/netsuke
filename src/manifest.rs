//! Manifest loading helpers.
//!
//! This module provides convenience functions for parsing a static
//! `Netsukefile` into the [`crate::ast::NetsukeManifest`] structure.
//! They wrap `serde_yml` and add basic file handling.

use crate::ast::NetsukeManifest;
use anyhow::{Context, Result};
use minijinja::{context, value::Value, Environment, UndefinedBehavior};
use serde::Deserialize;
use std::{collections::HashMap, fs, path::Path};

#[derive(Default, Deserialize)]
struct VarsOnly {
    #[serde(default)]
    vars: HashMap<String, String>,
}

/// Parse a manifest string using Jinja for templating.
///
/// The function renders the input YAML as a Jinja template, using any
/// top-level `vars` as the initial context, before parsing the expanded YAML
/// into a [`NetsukeManifest`].
///
/// # Examples
///
/// ```
/// use netsuke::manifest::from_str;
///
/// let yaml = r#"
/// netsuke_version: 1.0.0
/// vars:
///   who: world
/// targets:
///   - name: hello
///     command: echo {{ who }}
/// "#;
/// let manifest = from_str(yaml).expect("parse");
/// assert_eq!(manifest.targets.len(), 1);
/// ```
///
/// # Errors
///
/// Returns an error if Jinja rendering or YAML parsing fails.
pub fn from_str(yaml: &str) -> Result<NetsukeManifest> {
    // Bootstrap the template engine with lenient undefineds so we can extract
    // the global `vars` block without errors from unresolved placeholders.
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Lenient);

    // First pass: render the raw template to plain YAML, ignoring unresolved
    // expressions. This gives us access to the top-level `vars` mapping which
    // seeds the real render pass.
    let rendered = env
        .render_str(yaml, context! {})
        .context("Jinja render error")?;

    let vars_only: VarsOnly = serde_yml::from_str(&rendered).context("YAML parse error")?;

    // Populate the environment with the extracted variables for subsequent
    // rendering. Undefined variables now trigger errors to surface template
    // mistakes early.
    for (key, value) in &vars_only.vars {
        env.add_global(key, Value::from(value.clone()));
    }

    env.set_undefined_behavior(UndefinedBehavior::Strict);

    // Second pass: render the template again with the enriched context to
    // obtain a pure YAML manifest ready for deserialisation.
    let rendered = env
        .render_str(yaml, context! {})
        .context("Jinja render error")?;

    serde_yml::from_str::<NetsukeManifest>(&rendered).context("YAML parse error")
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
