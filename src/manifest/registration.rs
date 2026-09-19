//! Registers the manifest's template helpers and user-defined variables.
//!
//! The `env()` and `glob()` functions belong to the manifest layer rather than
//! the standard library, so they are bound here; `register_manifest_vars`
//! exposes the manifest's own `vars` section afterwards, refusing any variable
//! that would shadow a helper. The same module localizes the schema
//! diagnostics the registration paths produce.

use super::{
    EnvAccessPolicy, EnvReader, ManifestError, ManifestName, ManifestValue,
    env_reader::env_var_with,
    glob::{GlobBaseCache, expand_manifest_template_glob},
    map_data_error,
};
use crate::{
    ast::EMPTY_COMMAND_LIST_ERROR,
    localization::{self, keys},
};
use camino::Utf8PathBuf;
use minijinja::{Environment, value::Value};
use serde::de::Error as _;
use std::sync::Arc;

/// Names the manifest loader reserves for helper functions.
pub(super) const RESERVED_VAR_NAMES: [&str; 2] = ["env", "glob"];

/// Expose the `env()` helper, bounded by the access policy.
///
/// The reader and the policy are both cloned into the closure so the registered
/// helper outlives the parse inputs it was built from.
pub(super) fn register_env_function(
    jinja: &mut Environment<'_>,
    env_reader: &EnvReader,
    env_access_policy: &EnvAccessPolicy,
) {
    let reader = Arc::clone(env_reader);
    let policy_for_env_lookup = env_access_policy.clone();
    jinja.add_function("env", move |var_name: String| {
        env_var_with(&var_name, &policy_for_env_lookup, |key| reader(key))
    });
}

/// Expose the `glob()` helper, anchored at the manifest workspace root.
///
/// A `None` root leaves relative patterns anchored at the process current
/// directory, which is the composition root's fallback.
pub(super) fn register_glob_function(
    jinja: &mut Environment<'_>,
    manifest_root: Option<Utf8PathBuf>,
) {
    let glob_base = GlobBaseCache::new(manifest_root);
    jinja.add_function("glob", move |pattern: String| {
        let expansion = expand_manifest_template_glob(&pattern, &glob_base)?;
        expansion.into_template_paths(&pattern)
    });
}

/// Translate schema-only recipe errors at the manifest adapter boundary.
pub(super) fn localize_recipe_error(error: serde_json::Error) -> serde_json::Error {
    if error.to_string().starts_with(EMPTY_COMMAND_LIST_ERROR) {
        serde_json::Error::custom(
            localization::message(keys::MANIFEST_COMMAND_LIST_EMPTY).to_string(),
        )
    } else {
        error
    }
}

/// Expose the manifest's `vars` section as Jinja globals.
///
/// # Errors
///
/// Returns a structural manifest error for non-object variables or helper-name collisions.
pub(super) fn register_manifest_vars(
    doc: &ManifestValue,
    jinja: &mut Environment<'_>,
    name: &ManifestName,
) -> Result<(), ManifestError> {
    let Some(vars_value) = doc.get("vars") else {
        return Ok(());
    };
    let vars = vars_value.as_object().ok_or_else(|| {
        manifest_structure_error(&localization::message(keys::MANIFEST_VARS_NOT_OBJECT), name)
    })?;
    if let Some(reserved) = vars
        .keys()
        .find(|key| RESERVED_VAR_NAMES.contains(&key.as_str()))
    {
        return Err(manifest_structure_error(
            &localization::message(keys::MANIFEST_VARS_RESERVED_NAME).with_arg("name", reserved),
            name,
        ));
    }
    for (key, value) in vars {
        jinja.add_global(key.clone(), Value::from_serialize(value));
    }
    Ok(())
}

/// Build a structural manifest error with a localized message.
fn manifest_structure_error(
    detail: &localization::LocalizedMessage,
    name: &ManifestName,
) -> ManifestError {
    ManifestError::Parse {
        source: map_data_error(serde_json::Error::custom(detail.to_string()), name),
        message: localization::message(keys::MANIFEST_PARSE),
    }
}
