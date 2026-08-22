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
//! The optional `vars` section must deserialize into a JSON object; a list or
//! scalar is rejected with the localized `manifest.vars.not_object` diagnostic.
//! YAML mappings with non-string or composite keys cannot be represented as
//! JSON at all, so they fail earlier, during the initial `serde_saphyr` parse,
//! with the YAML parse diagnostic. Keys colliding with the built-in `env` and
//! `glob` helpers are rejected with the localized `manifest.vars.reserved_name`
//! diagnostic, since `MiniJinja` keeps functions and global variables in a
//! single namespace.

use crate::{
    ast::{EMPTY_COMMAND_LIST_ERROR, NetsukeManifest},
    localization::{self, keys},
    stdlib::{NetworkPolicy, StdlibConfig},
};
use anyhow::Result;
use minijinja::{Environment, UndefinedBehavior, value::Value};
use serde::de::Error as _;
use std::{path::Path, sync::Arc};

mod diagnostics;
mod expand;
// `glob_paths` is the module's only boundary: every other item, including the
// `GlobEntryResult` alias, stays module-private. Denying `unreachable_pub`
// here rejects `pub` items that are still unreachable from the crate root, so
// the boundary cannot regress silently. The `glob_paths` re-export below makes
// that one item genuinely reachable and therefore exempt; anything else
// widened to `pub` fails the build.
#[deny(unreachable_pub)]
mod glob;
mod hints;
mod jinja_macros;
mod parse_with_config;
mod query;
mod render;

/// JSON representation of a manifest node after YAML and Jinja evaluation.
pub type ManifestValue = serde_json::Value;
/// JSON object mapping string keys to manifest values.
pub type ManifestMap = serde_json::Map<String, ManifestValue>;

pub use diagnostics::{
    ManifestError, ManifestName, ManifestSource, map_data_error, map_yaml_error,
};
pub use env_reader::{EnvReadError, EnvReader, process_env_reader};
pub use glob::glob_paths;

pub(crate) use expand::expand_foreach;
pub use parse_with_config::from_str_with_env_and_config;
pub(crate) use query::from_path_for_manifest_query;
pub use render::render_manifest;

use self::{env_reader::env_var_with, jinja_macros::register_manifest_macros};
#[cfg(test)]
use workspace::open_manifest_workspace;

/// Stages in the manifest-loading sub-pipeline.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ManifestLoadStage {
    /// Read raw manifest content from the filesystem.
    ManifestIngestion,
    /// Parse raw YAML into a `serde_json::Value` tree.
    InitialYamlParsing,
    /// Expand `foreach` and `when` template directives.
    TemplateExpansion,
    /// Deserialize and render string fields into typed manifest data.
    FinalRendering,
}

/// Invoke the stage callback when present.
fn notify_stage(
    on_stage: &mut Option<&mut dyn FnMut(ManifestLoadStage)>,
    stage: ManifestLoadStage,
) {
    if let Some(cb) = on_stage.as_mut() {
        cb(stage);
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
/// Inputs to a manifest parse, bundled to keep the parameter list bounded.
struct ManifestParse<'a> {
    /// Name reported in diagnostics.
    name: &'a ManifestName,
    /// Optional stdlib registration configuration.
    stdlib_registration: Option<StdlibRegistration>,
    /// Environment reader backing the `env()` helper.
    env_reader: &'a EnvReader,
}

/// Selects the stdlib surface available while rendering a manifest.
enum StdlibRegistration {
    /// The complete stdlib used for a normal build manifest.
    Full(Box<StdlibConfig>),
    /// The read-only stdlib used to inspect manifest discovery metadata.
    ManifestQuery,
}
/// Parse a manifest string, running the full YAML, Jinja and expansion pipeline.
fn from_str_named(
    yaml: &str,
    parse: ManifestParse<'_>,
    on_stage: &mut Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    let ManifestParse {
        name,
        stdlib_registration,
        env_reader,
    } = parse;
    notify_stage(on_stage, ManifestLoadStage::InitialYamlParsing);
    let mut doc: ManifestValue =
        serde_saphyr::from_str(yaml).map_err(|e| ManifestError::Parse {
            source: map_yaml_error(e, &ManifestSource::from(yaml), name),
            message: localization::message(keys::MANIFEST_PARSE),
        })?;

    let mut jinja = Environment::new();
    jinja.set_undefined_behavior(UndefinedBehavior::Strict);
    // Expose custom helpers to templates.
    let reader = Arc::clone(env_reader);
    jinja.add_function("env", move |var_name: String| {
        env_var_with(&var_name, |key| reader(key))
    });
    jinja.add_function("glob", |pattern: String| {
        let expansion = glob::expand_glob(&pattern)?;
        glob::record_expansion(&expansion);
        Ok(expansion.into_paths())
    });
    let _stdlib_state = match stdlib_registration {
        Some(StdlibRegistration::Full(config)) => {
            crate::stdlib::register_with_config(&mut jinja, *config)
        }
        Some(StdlibRegistration::ManifestQuery) => {
            Ok(crate::stdlib::register_manifest_query(&mut jinja))
        }
        None => crate::stdlib::register(&mut jinja),
    }?;

    register_manifest_vars(&doc, &mut jinja, name)?;

    notify_stage(on_stage, ManifestLoadStage::TemplateExpansion);
    register_manifest_macros(&doc, &mut jinja)?;

    expand_foreach(&mut doc, &jinja)?;

    notify_stage(on_stage, ManifestLoadStage::FinalRendering);
    let manifest: NetsukeManifest =
        serde_json::from_value(doc).map_err(|error| ManifestError::Parse {
            source: map_data_error(localize_recipe_error(error), name),
            message: localization::message(keys::MANIFEST_PARSE),
        })?;

    render_manifest(manifest, &jinja)
}

/// Translate schema-only recipe errors at the manifest adapter boundary.
fn localize_recipe_error(error: serde_json::Error) -> serde_json::Error {
    if error.to_string().starts_with(EMPTY_COMMAND_LIST_ERROR) {
        serde_json::Error::custom(
            localization::message(keys::MANIFEST_COMMAND_LIST_EMPTY).to_string(),
        )
    } else {
        error
    }
}

/// Names the manifest loader registers as Jinja helper functions.
///
/// `MiniJinja` keeps functions and global variables in a single namespace, so a
/// `vars` entry using one of these names would silently replace the helper and
/// break every template that calls it.
const RESERVED_VAR_NAMES: [&str; 2] = ["env", "glob"];

/// Build a [`ManifestError::Parse`] carrying a localized structural diagnostic.
fn manifest_structure_error(
    detail: &localization::LocalizedMessage,
    name: &ManifestName,
) -> ManifestError {
    ManifestError::Parse {
        source: map_data_error(serde_json::Error::custom(detail.to_string()), name),
        message: localization::message(keys::MANIFEST_PARSE),
    }
}

/// Expose the manifest's `vars` section as Jinja globals.
///
/// The optional `vars` value must be a JSON object; each entry becomes a global
/// available to every template expression evaluated for this manifest. For
/// example, given `vars: {greeting: hi}`, a target command of
/// `"echo {{ greeting }}"` renders to `echo hi`.
///
/// # Errors
///
/// Returns [`ManifestError::Parse`] when `vars` is present but is not an
/// object (for example a list or a scalar), or when a key collides with one of
/// the [`RESERVED_VAR_NAMES`] helper functions.
fn register_manifest_vars(
    doc: &ManifestValue,
    jinja: &mut Environment<'_>,
    name: &ManifestName,
) -> Result<(), ManifestError> {
    let Some(vars_value) = doc.get("vars") else {
        return Ok(());
    };
    // Borrow the map rather than cloning it: only the key needs to be owned,
    // because `add_global` stores a `Cow<'source, str>` that cannot borrow from
    // the caller's document.
    let vars = vars_value.as_object().ok_or_else(|| {
        manifest_structure_error(&localization::message(keys::MANIFEST_VARS_NOT_OBJECT), name)
    })?;
    // Reject collisions before registering anything, so a rejected manifest
    // never leaves the environment half-populated.
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

/// Parse a manifest string using Jinja for value templating.
///
/// The input YAML must be valid on its own. Jinja expressions are evaluated
/// only inside recognised string fields and the `foreach` and `when` keys.
///
/// # Errors
///
/// Returns an error if YAML parsing or Jinja evaluation fails.
pub fn from_str(yaml: &str) -> Result<NetsukeManifest> {
    from_str_with_env(yaml, &process_env_reader())
}

/// Parse a manifest string with an explicit environment reader.
///
/// Lets a caller — in practice a test — drive the `env()` helper without
/// touching the process environment.
///
/// # Errors
///
/// Returns an error if YAML parsing or Jinja evaluation fails.
///
/// # Examples
///
/// ```
/// use netsuke::{
///     ast::Recipe,
///     manifest::{EnvReadError, EnvReader, from_str_with_env},
/// };
/// use std::sync::Arc;
///
/// let reader: EnvReader = Arc::new(|name| match name {
///     "PROFILE" => Ok("release".to_owned()),
///     _ => Err(EnvReadError::NotPresent),
/// });
/// let yaml = concat!(
///     "netsuke_version: 1.0.0\n",
///     "targets:\n",
///     "  - name: build\n",
///     "    command: echo {{ env('PROFILE') }}\n",
/// );
/// let manifest = from_str_with_env(yaml, &reader).expect("parse manifest");
///
/// assert!(matches!(
///     &manifest.targets[0].recipe,
///     Recipe::Command { command } if command.as_single() == Some("echo release")
/// ));
/// ```
pub fn from_str_with_env(yaml: &str, env_reader: &EnvReader) -> Result<NetsukeManifest> {
    from_str_named(
        yaml,
        ManifestParse {
            name: &ManifestName::new("Netsukefile"),
            stdlib_registration: None,
            env_reader,
        },
        &mut None,
    )
}

/// Load a [`NetsukeManifest`] from the given file path.
///
/// # Errors
///
/// Returns an error if the file cannot be read or the YAML fails to parse.
pub fn from_path(path: impl AsRef<Path>) -> Result<NetsukeManifest> {
    from_path_with_policy(path, NetworkPolicy::default(), None)
}

/// Load a [`NetsukeManifest`] from the given file path using an explicit
/// network policy and an optional stage callback.
///
/// The callback, when provided, is invoked in order for each manifest stage.
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
/// let manifest = manifest::from_path_with_policy("Netsukefile", policy, None);
/// assert!(manifest.is_ok());
/// ```
pub fn from_path_with_policy(
    path: impl AsRef<Path>,
    policy: NetworkPolicy,
    on_stage: Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    from_path_with_policy_and_env(path, policy, &process_env_reader(), on_stage)
}

/// Load a manifest with explicit network policy and environment reader.
///
/// This adapter boundary lets callers supply deterministic manifest variables
/// without mutating the process environment.
///
/// # Errors
///
/// Returns an error if the manifest cannot be read, rendered, or parsed.
///
/// # Examples
///
/// ```
/// use netsuke::{
///     ast::Recipe,
///     manifest::{EnvReadError, EnvReader, from_path_with_policy_and_env},
///     stdlib::NetworkPolicy,
/// };
/// use std::{io::Write, sync::Arc};
///
/// let mut file = tempfile::NamedTempFile::new().expect("create manifest");
/// write!(
///     file,
///     "netsuke_version: 1.0.0\ntargets:\n  - name: build\n    command: echo {{{{ env('PROFILE') }}}}\n"
/// )
/// .expect("write manifest");
/// let reader: EnvReader = Arc::new(|name| match name {
///     "PROFILE" => Ok("offline".to_owned()),
///     _ => Err(EnvReadError::NotPresent),
/// });
/// let policy = NetworkPolicy::default().deny_all_hosts();
/// let manifest = from_path_with_policy_and_env(file.path(), policy, &reader, None)
///     .expect("load manifest without network access");
///
/// assert!(matches!(
///     &manifest.targets[0].recipe,
///     Recipe::Command { command } if command.as_single() == Some("echo offline")
/// ));
/// ```
pub fn from_path_with_policy_and_env(
    path: impl AsRef<Path>,
    policy: NetworkPolicy,
    env_reader: &EnvReader,
    on_stage: Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    query::from_path_with_policy_and_env(path, policy, env_reader, on_stage)
}

mod env_reader;
mod workspace;

#[cfg(test)]
mod tests;
