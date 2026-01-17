//! Manifest loading helpers.
//!
//! This module parses a `Netsukefile` without relying on a global Jinja
//! preprocessing pass. The YAML is parsed first and Jinja expressions are
//! evaluated only within string values or the `foreach` and `when` keys. It
//! exposes `env()` to read environment variables and `glob()` to expand
//! filesystem patterns during template evaluation. Both helpers fail fast when
//! inputs are missing or patterns are invalid.
//!
//! Consumers interact with the intermediate manifest through the re-exported
//! [`ManifestValue`] and [`ManifestMap`] aliases. Diagnostics wrap manifest
//! identifiers in [`ManifestName`] and YAML source strings in
//! [`ManifestSource`] so callers pass domain-specific types instead of raw
//! strings.
//!
//! The optional `vars` section must deserialise into a JSON object with string
//! keys. YAML manifests that use non-string keys (for example integers) now
//! fail with a [`ManifestError::Parse`] diagnostic, matching the Jinja context
//! semantics and preventing ambiguous variable lookup.

use crate::{
    ast::NetsukeManifest,
    localization::{self, keys},
    stdlib::{NetworkPolicy, StdlibConfig},
};
use anyhow::{Context, Result, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, Error, ErrorKind, UndefinedBehavior, value::Value};
use serde::de::Error as _;
use std::{env, fs, path::Path};

mod diagnostics;
mod expand;
mod glob;
mod hints;
mod jinja_macros;
mod render;

/// JSON representation of a manifest node after YAML and Jinja evaluation.
pub type ManifestValue = serde_json::Value;
/// JSON object mapping string keys to manifest values.
pub type ManifestMap = serde_json::Map<String, ManifestValue>;

pub use diagnostics::{
    ManifestError, ManifestName, ManifestSource, map_data_error, map_yaml_error,
};
pub use glob::glob_paths;

pub use expand::expand_foreach;
pub use render::render_manifest;

use self::jinja_macros::register_manifest_macros;

/// Resolve the value of an environment variable for the `env()` Jinja helper.
///
/// Returns the variable's value or a structured error that mirrors Jinja's
/// failure modes, ensuring templates halt when a variable is missing or not
/// valid UTF-8.
///
/// # Examples
///
/// The [`EnvLock`](test_support::env_lock::EnvLock) guard serialises access to
/// the process environment so tests do not interfere with each other.
///
/// ```rust,ignore
/// use test_support::env_lock::EnvLock;
/// let _guard = EnvLock::acquire();
/// std::env::set_var("FOO", "bar");
/// assert_eq!(env("FOO").unwrap(), "bar");
/// std::env::remove_var("FOO");
/// ```
fn env_var(name: &str) -> std::result::Result<String, Error> {
    match std::env::var(name) {
        Ok(val) => Ok(val),
        Err(std::env::VarError::NotPresent) => Err(Error::new(
            ErrorKind::UndefinedError,
            localization::message(keys::MANIFEST_ENV_MISSING)
                .with_arg("name", name)
                .to_string(),
        )),
        Err(std::env::VarError::NotUnicode(_)) => Err(Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::MANIFEST_ENV_INVALID_UTF8)
                .with_arg("name", name)
                .to_string(),
        )),
    }
}

/// Parse a manifest string using Jinja for value templating.
///
/// The input YAML must be valid on its own. Jinja expressions are evaluated
/// only inside recognised string fields and the `foreach` and `when` keys.
///
/// # Errors
///
/// Returns an error if YAML parsing or Jinja evaluation fails.
fn from_str_named(
    yaml: &str,
    name: &ManifestName,
    stdlib_config: Option<StdlibConfig>,
) -> Result<NetsukeManifest> {
    let mut doc: ManifestValue =
        serde_saphyr::from_str(yaml).map_err(|e| ManifestError::Parse {
            source: map_yaml_error(e, &ManifestSource::from(yaml), name),
            message: localization::message(keys::MANIFEST_PARSE),
        })?;

    let mut jinja = Environment::new();
    jinja.set_undefined_behavior(UndefinedBehavior::Strict);
    // Expose custom helpers to templates.
    jinja.add_function("env", |var_name: String| env_var(&var_name));
    jinja.add_function("glob", |pattern: String| glob_paths(&pattern));
    let _stdlib_state = match stdlib_config {
        Some(config) => crate::stdlib::register_with_config(&mut jinja, config),
        None => crate::stdlib::register(&mut jinja),
    }?;

    if let Some(vars_value) = doc.get("vars") {
        let vars = vars_value
            .as_object()
            .cloned()
            .ok_or_else(|| ManifestError::Parse {
                source: map_data_error(
                    serde_json::Error::custom(
                        localization::message(keys::MANIFEST_VARS_NOT_OBJECT).to_string(),
                    ),
                    name,
                ),
                message: localization::message(keys::MANIFEST_PARSE),
            })?;
        for (key, value) in vars {
            jinja.add_global(key, Value::from_serialize(value));
        }
    }

    register_manifest_macros(&doc, &mut jinja)?;

    expand_foreach(&mut doc, &jinja)?;

    let manifest: NetsukeManifest =
        serde_json::from_value(doc).map_err(|e| ManifestError::Parse {
            source: map_data_error(e, name),
            message: localization::message(keys::MANIFEST_PARSE),
        })?;

    render_manifest(manifest, &jinja)
}

/// Parse a manifest string using Jinja for value templating.
///
/// The input YAML must be valid on its own. Jinja expressions are evaluated
/// only inside recognised string fields and the `foreach` and `when` keys.
///
/// # Errors
///
/// Returns an error if YAML parsing or Jinja evaluation fails.
pub fn from_str(yaml: &str) -> Result<NetsukeManifest> {
    from_str_named(yaml, &ManifestName::new("Netsukefile"), None)
}

/// Load a [`NetsukeManifest`] from the given file path.
///
/// # Errors
///
/// Returns an error if the file cannot be read or the YAML fails to parse.
pub fn from_path(path: impl AsRef<Path>) -> Result<NetsukeManifest> {
    from_path_with_policy(path, NetworkPolicy::default())
}

/// Load a [`NetsukeManifest`] from the given file path using an explicit network
/// policy.
///
/// # Errors
///
/// Returns an error if the file cannot be read or the YAML fails to parse.
///
/// # Examples
///
/// ```rust,ignore
/// use netsuke::manifest;
/// use netsuke::stdlib::NetworkPolicy;
///
/// let policy = NetworkPolicy::default();
/// let manifest = manifest::from_path_with_policy("Netsukefile", policy);
/// assert!(manifest.is_ok());
/// ```
pub fn from_path_with_policy(
    path: impl AsRef<Path>,
    policy: NetworkPolicy,
) -> Result<NetsukeManifest> {
    let path_ref = path.as_ref();
    let data = fs::read_to_string(path_ref).with_context(|| {
        localization::message(keys::MANIFEST_READ_FAILED)
            .with_arg("path", path_ref.display().to_string())
    })?;
    let name = ManifestName::new(path_ref.display().to_string());
    let config = stdlib_config_for_manifest(path_ref, policy)?;
    from_str_named(&data, &name, Some(config))
}

#[cfg(test)]
mod tests;

/// Resolve a potentially relative manifest parent path to an absolute UTF-8 workspace root.
fn resolve_absolute_workspace_root(utf8_parent: &Utf8Path) -> Result<Utf8PathBuf> {
    let workspace_base = if utf8_parent.is_absolute() {
        utf8_parent.to_path_buf().into_std_path_buf()
    } else {
        env::current_dir()
            .context(localization::message(keys::MANIFEST_RESOLVE_WORKSPACE_ROOT))?
            .join(utf8_parent.as_std_path())
    };
    Utf8PathBuf::from_path_buf(workspace_base).map_err(|invalid| {
        anyhow!(
            "{}",
            localization::message(keys::MANIFEST_WORKSPACE_NON_UTF8)
                .with_arg("path", invalid.display().to_string())
        )
    })
}

fn stdlib_config_for_manifest(path: &Path, policy: NetworkPolicy) -> Result<StdlibConfig> {
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let manifest_label = path
        .file_name()
        .and_then(|name| name.to_str())
        .map_or_else(|| path.display().to_string(), str::to_owned);
    let utf8_parent = Utf8Path::from_path(parent).ok_or_else(|| {
        anyhow!(
            "{}",
            localization::message(keys::MANIFEST_PATH_NON_UTF8)
                .with_arg("manifest", &manifest_label)
                .with_arg("path", path.display().to_string())
        )
    })?;
    let workspace_root = resolve_absolute_workspace_root(utf8_parent)?;
    let dir = Dir::open_ambient_dir(workspace_root.as_path(), ambient_authority()).with_context(
        || {
            localization::message(keys::MANIFEST_OPEN_WORKSPACE_FAILED)
                .with_arg("workspace", workspace_root.as_str())
                .with_arg("manifest", &manifest_label)
        },
    )?;
    Ok(StdlibConfig::new(dir)?
        .with_workspace_root_path(workspace_root)?
        .with_network_policy(policy))
}
