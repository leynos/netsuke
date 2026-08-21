//! Discovery of configuration file layers.
//!
//! Runs the `OrthoConfig` scan and applies the project-scope second pass, so a
//! project `.netsuke.toml` outranks user-scope files. Path comparison and its
//! fallback policy live here because that policy is a discovery decision.

#[cfg(test)]
use ortho_config::MapEnv;
use ortho_config::{
    ConfigDiscovery, MergeLayer, MergeProvenance, OrthoResult, SharedEnvSource,
    load_config_file_as_chain,
};
use std::borrow::Cow;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::Arc;

use super::super::parser::Cli;
use super::CONFIG_ENV_VAR;
use super::diagnostics::{
    BoundedConfigPath, debug_optional_config_path_from_fields, debug_project_layer_deduplication,
};
use super::json::json_from_value;
use super::paths::{FsPathNormalizer, PathNormalizer, normalized_path_key};

/// Preserve discovered layers while extracting their final JSON preference.
///
/// `MergeLayer` exposes its value only through consuming access. Rebuilding a
/// file layer after inspecting that owned value avoids cloning a complete JSON
/// configuration tree before the cached merge consumes it.
pub(super) fn retain_layers_and_resolve_json(
    layers: Vec<MergeLayer<'static>>,
) -> (Vec<MergeLayer<'static>>, bool) {
    let mut json = Cli::default().json;
    let mut retained = Vec::with_capacity(layers.len());
    for layer in layers {
        debug_assert_eq!(
            layer.provenance(),
            MergeProvenance::File,
            "discovery must retain only file layers"
        );
        let path = layer.path().map(ToOwned::to_owned);
        let value = layer.into_value();
        if let Some(layer_json) = json_from_value(&value) {
            json = layer_json;
        }
        retained.push(MergeLayer::file(Cow::Owned(value), path));
    }
    (retained, json)
}

/// Project-scope outcome retained for a later trace replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ProjectScopeTrace {
    /// The primary discovery scan already yielded the project configuration.
    Included(BoundedConfigPath),
    /// The project configuration was loaded by the second pass.
    Appended(BoundedConfigPath),
    /// The second pass found only layers already returned by discovery.
    Deduplicated(BoundedConfigPath),
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
            Self::Deduplicated(path) => {
                debug_optional_config_path_from_fields(
                    "project-scope layers already discovered",
                    path,
                );
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

/// Return the key used to compare the expected project file against a layer.
///
/// This is the discovery-side fallback policy for [`normalized_path_key`]. A
/// path that cannot be resolved — most often the expected `.netsuke.toml`, which
/// frequently does not exist, or a directory the process cannot read — is
/// compared literally with `OrthoConfig`'s already-canonicalized layer path
/// rather than failing discovery. An exact textual match still identifies the layer;
/// otherwise the project-scope pass de-duplicates loaded layers and retains
/// its bounded outcome for the composition boundary.
fn comparison_key(normalizer: &impl PathNormalizer, path: &str) -> PathBuf {
    normalized_path_key(normalizer, path).unwrap_or_else(|_| PathBuf::from(path))
}

/// Build the discovery chain and its bounded project-scope trace metadata.
#[cfg(test)]
pub(super) fn collect_file_layers_with_normalizer(
    directory: Option<&Path>,
    normalizer: &impl PathNormalizer,
) -> OrthoResult<Vec<MergeLayer<'static>>> {
    let isolated_config_dirs = directory.map_or_else(
        || PathBuf::from(".netsuke-test-absent-xdg-config-dirs"),
        |path| path.join(".netsuke-test-absent-xdg-config-dirs"),
    );
    let mut test_env = MapEnv::new();
    // Leave selector and home variables unset so this test cannot inherit the
    // host configuration; XDG_CONFIG_DIRS points at an isolated absent path.
    test_env.insert("XDG_CONFIG_DIRS", isolated_config_dirs.into_os_string());
    collect_file_layers_with_normalizer_and_trace(directory, normalizer, Arc::new(test_env)).1
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
    let lossy_project_key = project_key
        .as_deref()
        .filter(|path| path.to_str().is_none())
        .map(Path::to_string_lossy);
    let has_project_layer = project_key.as_deref().is_some_and(|key| {
        file_layers.value.iter().any(|layer| {
            layer.path().is_some_and(|path| {
                key == path.as_std_path()
                    || lossy_project_key
                        .as_deref()
                        .is_some_and(|lossy_key| lossy_key == path.as_str())
            })
        })
    });
    let project_trace_path = BoundedConfigPath::from_path(project_file.as_deref());
    if has_project_layer {
        return (
            Some(ProjectScopeTrace::Included(project_trace_path)),
            Ok(file_layers.value),
        );
    }

    merge_project_scope_layers(
        file_layers.value,
        project_file.as_deref(),
        project_trace_path,
    )
}

/// Load project layers, filter aliases already yielded by discovery, and trace the outcome.
fn merge_project_scope_layers(
    discovered_layers: Vec<MergeLayer<'static>>,
    project_file: Option<&Path>,
    project_trace_path: BoundedConfigPath,
) -> (
    Option<ProjectScopeTrace>,
    OrthoResult<Vec<MergeLayer<'static>>>,
) {
    let error_trace = ProjectScopeTrace::Appended(project_trace_path.clone());
    let result = project_scope_layers(project_file).map(|project_layers| {
        let discovered_paths = discovered_layers
            .iter()
            .filter_map(|layer| layer.path().map(camino::Utf8Path::as_str))
            .collect::<HashSet<_>>();
        let discovered_layer_count = discovered_paths.len();
        let project_layer_count = project_layers.len();
        let project_layers_to_append = project_layers
            .into_iter()
            .filter(|layer| {
                layer
                    .path()
                    .is_none_or(|path| !discovered_paths.contains(path.as_str()))
            })
            .collect::<Vec<_>>();
        let appended_layer_count = project_layers_to_append.len();
        debug_project_layer_deduplication(
            discovered_layer_count,
            project_layer_count,
            appended_layer_count,
        );
        let trace = if project_layer_count == 0 {
            None
        } else if appended_layer_count == 0 {
            Some(ProjectScopeTrace::Deduplicated(project_trace_path))
        } else {
            Some(ProjectScopeTrace::Appended(project_trace_path))
        };
        let layers = discovered_layers
            .into_iter()
            .chain(project_layers_to_append)
            .collect();
        (trace, layers)
    });
    match result {
        Ok((trace, layers)) => (trace, Ok(layers)),
        Err(err) => (Some(error_trace), Err(err)),
    }
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
