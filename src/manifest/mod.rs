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
//! The optional `vars` section must deserialize into a JSON object with string
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
use minijinja::{Environment, UndefinedBehavior, value::Value};
use serde::de::Error as _;
use std::{env, path::Path, sync::Arc};

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
pub use env_reader::{EnvReader, process_env_reader};
pub use glob::glob_paths;

pub(crate) use expand::expand_foreach;
pub use render::render_manifest;

use self::{env_reader::env_var_with, jinja_macros::register_manifest_macros};

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
    /// Optional stdlib configuration override.
    stdlib_config: Option<StdlibConfig>,
    /// Environment reader backing the `env()` helper.
    env_reader: &'a EnvReader,
}

fn from_str_named(
    yaml: &str,
    parse: ManifestParse<'_>,
    on_stage: &mut Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    let ManifestParse {
        name,
        stdlib_config,
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

    notify_stage(on_stage, ManifestLoadStage::TemplateExpansion);
    register_manifest_macros(&doc, &mut jinja)?;

    expand_foreach(&mut doc, &jinja)?;

    notify_stage(on_stage, ManifestLoadStage::FinalRendering);
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
/// ```rust
/// use netsuke::manifest::{EnvReader, from_str_with_env};
/// use std::sync::Arc;
///
/// let reader: EnvReader = Arc::new(|_| Ok(String::from("release")));
/// let yaml = concat!(
///     "netsuke_version: \"1.0.0\"\n",
///     "targets:\n",
///     "  - name: \"{{ env('PROFILE') }}\"\n",
///     "    command: echo hi\n",
/// );
/// let manifest = from_str_with_env(yaml, &reader).expect("parse");
/// assert!(format!("{:?}", manifest.targets[0].name).contains("release"));
/// ```
pub fn from_str_with_env(yaml: &str, env_reader: &EnvReader) -> Result<NetsukeManifest> {
    from_str_named(
        yaml,
        ManifestParse {
            name: &ManifestName::new("Netsukefile"),
            stdlib_config: None,
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
    mut on_stage: Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    notify_stage(&mut on_stage, ManifestLoadStage::ManifestIngestion);
    let path_ref = path.as_ref();
    let workspace = open_manifest_workspace(path_ref)?;
    let data = workspace
        .dir
        .read_to_string(&workspace.manifest_file)
        .with_context(|| {
            localization::message(keys::MANIFEST_READ_FAILED)
                .with_arg("path", path_ref.display().to_string())
        })?;
    let name = ManifestName::new(path_ref.display().to_string());
    let config = StdlibConfig::new(workspace.dir)?
        .with_workspace_root_path(workspace.root)?
        .with_network_policy(policy);
    from_str_named(
        &data,
        ManifestParse {
            name: &name,
            stdlib_config: Some(config),
            env_reader: &process_env_reader(),
        },
        &mut on_stage,
    )
}

mod env_reader;

#[cfg(test)]
mod tests;

/// Resolve a potentially relative manifest parent path to an absolute UTF-8 workspace root.
fn resolve_absolute_workspace_root(utf8_parent: &Utf8Path) -> Result<Utf8PathBuf> {
    let workspace_base = if utf8_parent.is_absolute() {
        utf8_parent.to_path_buf().into_std_path_buf()
    } else {
        std_env::current_dir()
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

/// A manifest's workspace opened for capability-based access.
#[derive(Debug)]
struct ManifestWorkspace {
    /// Capability-scoped handle on the workspace root.
    dir: Dir,
    /// Absolute UTF-8 path of the workspace root.
    root: Utf8PathBuf,
    /// Manifest file name relative to the workspace root.
    manifest_file: String,
}

/// Open the directory containing `path` as a capability-scoped workspace.
fn open_manifest_workspace(path: &Path) -> Result<ManifestWorkspace> {
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let manifest_file = match path.file_name() {
        None => {
            return Err(anyhow!(
                "{}",
                localization::message(keys::MANIFEST_PATH_MISSING_NAME)
                    .with_arg("path", path.display().to_string())
            ));
        }
        Some(name) => name.to_str().map(str::to_owned).ok_or_else(|| {
            anyhow!(
                "{}",
                localization::message(keys::MANIFEST_PATH_NON_UTF8)
                    .with_arg("manifest", path.display().to_string())
                    .with_arg("path", path.display().to_string())
            )
        })?,
    };
    let utf8_parent = Utf8Path::from_path(parent).ok_or_else(|| {
        anyhow!(
            "{}",
            localization::message(keys::MANIFEST_PATH_NON_UTF8)
                .with_arg("manifest", &manifest_file)
                .with_arg("path", path.display().to_string())
        )
    })?;
    let root = resolve_absolute_workspace_root(utf8_parent)?;
    tracing::debug!(workspace = %root, manifest = %manifest_file, "opening manifest workspace directory");
    let dir = Dir::open_ambient_dir(root.as_path(), ambient_authority())
        .inspect_err(|err| {
            tracing::warn!(workspace = %root, manifest = %manifest_file, error = %err, "failed to open manifest workspace directory");
        })
        .with_context(|| {
            localization::message(keys::MANIFEST_OPEN_WORKSPACE_FAILED)
                .with_arg("workspace", root.as_str())
                .with_arg("manifest", &manifest_file)
        })?;
    Ok(ManifestWorkspace {
        dir,
        root,
        manifest_file,
    })
}
