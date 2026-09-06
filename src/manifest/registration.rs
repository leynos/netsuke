//! Registers user-defined manifest variables and localizes schema diagnostics.

use super::{ManifestError, ManifestName, ManifestValue, map_data_error};
use crate::{
    ast::EMPTY_COMMAND_LIST_ERROR,
    localization::{self, keys},
};
use minijinja::{Environment, value::Value};
use serde::de::Error as _;

/// Names the manifest loader reserves for helper functions.
pub(super) const RESERVED_VAR_NAMES: [&str; 2] = ["env", "glob"];

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
