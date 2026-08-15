//! Discovery of configuration file layers.
//!
//! Runs the `OrthoConfig` scan and applies the project-scope second pass, so a
//! project `.netsuke.toml` outranks user-scope files. Path comparison and its
//! fallback policy live here because that policy is a discovery decision.

use ortho_config::{
    ConfigDiscovery, MergeLayer, OrthoResult, SharedEnvSource, load_config_file_as_chain,
};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::CONFIG_ENV_VAR;
use super::diagnostics::{BoundedConfigPath, debug_optional_config_path_from_fields};
use super::paths::{FsPathNormalizer, PathNormalizer, normalized_path_key};

/// Project-scope outcome retained for a later trace replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ProjectScopeTrace {
    /// The primary discovery scan already yielded the project configuration.
    Included(BoundedConfigPath),
    /// The project configuration was loaded by the second pass.
    Appended(BoundedConfigPath),
}

impl ProjectScopeTrace {
    /// Replay the original project-scope diagnostic from bounded metadata.
    pub(super) fn emit(&self) {
        match self {
            Self::Included(path) => {
                debug_optional_config_path_from_fields(
                    "discovery included project-scope layers",
                    path,
                );
            }
            Self::Appended(path) => {
                debug_optional_config_path_from_fields("appending project-scope layers", path);
            }
        }
    }
}

/// Build the single-pass `OrthoConfig` discovery scanner.
///
/// Anchors the project root to `directory` when supplied; otherwise the default
/// project roots apply. `NETSUKE_CONFIG` is registered as the discovery env var.
fn config_discovery(directory: Option<&PathBuf>, env_source: SharedEnvSource) -> ConfigDiscovery {
    let mut builder = ConfigDiscovery::builder("netsuke")
        .env_var(CONFIG_ENV_VAR)
        .env_source(env_source);
    if let Some(dir) = directory {
        builder = builder.clear_project_roots().add_project_root(dir);
    }
    builder.build()
}

/// Run discovery once and retain the project-scope outcome for later replay.
pub(super) fn collect_file_layers_with_trace(
    directory: Option<&Path>,
) -> (
    Option<ProjectScopeTrace>,
    OrthoResult<Vec<MergeLayer<'static>>>,
) {
    collect_file_layers_with_trace_and_env_source(directory, Arc::new(ortho_config::ProcessEnv))
}

/// Run discovery with the composition root's environment source and retain its
/// project-scope outcome for later replay.
pub(super) fn collect_file_layers_with_trace_and_env_source(
    directory: Option<&Path>,
    env_source: SharedEnvSource,
) -> (
    Option<ProjectScopeTrace>,
    OrthoResult<Vec<MergeLayer<'static>>>,
) {
    collect_file_layers_with_normalizer_and_trace(directory, &FsPathNormalizer, env_source)
}

/// Return the key used to compare `path` against the expected project file.
///
/// This is the discovery-side fallback policy for [`normalized_path_key`]. A
/// path that cannot be resolved — most often the expected `.netsuke.toml`, which
/// frequently does not exist, or a directory the process cannot read — is
/// compared literally with `OrthoConfig`'s already-canonicalized layer path
/// rather than failing discovery. An exact textual match still identifies the layer;
/// otherwise the project-scope pass appends it and retains its normal debug
/// event for the composition boundary.
fn comparison_key(normalizer: &impl PathNormalizer, path: &str) -> PathBuf {
    normalized_path_key(normalizer, path).unwrap_or_else(|_| PathBuf::from(path))
}

/// Build the discovery chain and its bounded project-scope trace metadata.
#[cfg(test)]
pub(super) fn collect_file_layers_with_normalizer(
    directory: Option<&Path>,
    normalizer: &impl PathNormalizer,
) -> OrthoResult<Vec<MergeLayer<'static>>> {
    collect_file_layers_with_normalizer_and_trace(
        directory,
        normalizer,
        Arc::new(ortho_config::ProcessEnv),
    )
    .1
}

/// Build the discovery chain and project-scope trace using `normalizer`.
fn collect_file_layers_with_normalizer_and_trace(
    directory: Option<&Path>,
    normalizer: &impl PathNormalizer,
    env_source: SharedEnvSource,
) -> (
    Option<ProjectScopeTrace>,
    OrthoResult<Vec<MergeLayer<'static>>>,
) {
    let discovery = config_discovery(directory.map(PathBuf::from).as_ref(), env_source);
    let mut file_layers = discovery.compose_layers();
    let mut errors = file_layers.required_errors;
    if file_layers.value.is_empty() {
        errors.append(&mut file_layers.optional_errors);
    }
    if let Some(err) = errors.into_iter().next() {
        return (None, Err(err));
    }

    let project_file = project_scope_file(directory);
    let project_key = project_file
        .as_deref()
        .map(|path| comparison_key(normalizer, &path.to_string_lossy()));
    let has_project_layer = file_layers.value.iter().any(|layer| {
        layer.path().is_some_and(|path| {
            project_key
                .as_deref()
                .is_some_and(|key| key.to_string_lossy() == path.as_str())
        })
    });
    let project_trace_path = BoundedConfigPath::from_path(project_file.as_deref());
    if has_project_layer {
        return (
            Some(ProjectScopeTrace::Included(project_trace_path)),
            Ok(file_layers.value),
        );
    }

    let trace = ProjectScopeTrace::Appended(project_trace_path);
    let result = project_scope_layers(project_file.as_deref()).map(|project_layers| {
        file_layers
            .value
            .into_iter()
            .chain(project_layers)
            .collect()
    });
    (Some(trace), result)
}

fn project_scope_file(directory: Option<&Path>) -> Option<PathBuf> {
    let root = directory
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())?;
    Some(root.join(".netsuke.toml"))
}

fn project_scope_layers(project_file: Option<&Path>) -> OrthoResult<Vec<MergeLayer<'static>>> {
    let Some(path) = project_file else {
        return Ok(Vec::new());
    };
    match load_config_file_as_chain(path) {
        Ok(Some(chain)) => Ok(chain
            .values
            .into_iter()
            .map(|(value, layer_path)| MergeLayer::file(Cow::Owned(value), Some(layer_path)))
            .collect()),
        Ok(None) => Ok(Vec::new()),
        Err(err) => Err(err),
    }
}
