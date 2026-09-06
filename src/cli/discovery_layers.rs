//! Discovery of configuration file layers.
//!
//! Runs the `OrthoConfig` scan and applies the project-scope second pass, so a
//! project `.netsuke.toml` outranks user-scope files. Path comparison and its
//! fallback policy live here because that policy is a discovery decision.

#[cfg(test)]
use ortho_config::MapEnv;
use ortho_config::{
    ConfigDiscovery, MergeLayer, OrthoResult, SharedEnvSource, load_config_file_as_chain,
};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::Arc;

use super::CONFIG_ENV_VAR;
use super::diagnostics::{
    BoundedConfigPath, ProjectLayerDeduplication, debug_optional_config_path_from_fields,
};
use super::paths::{PathNormalizer, comparison_key, project_scope_file};
use super::project_policy::scope_primary_project_layer;
pub(super) use super::project_policy::{
    ScopedFileLayer, retain_layers_and_resolve_json, scope_selected_primary_layer,
};

/// Project-scope outcome retained for a later trace replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ProjectScopeTrace {
    /// The primary discovery scan already yielded the project configuration.
    Included(BoundedConfigPath),
    /// The project configuration was loaded by the second pass.
    Appended {
        /// Project config path loaded by the fallback pass.
        path: BoundedConfigPath,
        /// Bounded de-duplication outcome, when one was recorded.
        deduplication: Option<ProjectLayerDeduplication>,
    },
    /// The second pass found only layers already returned by discovery.
    Deduplicated {
        /// Project config path already returned by the primary scan.
        path: BoundedConfigPath,
        /// Bounded de-duplication outcome for the repeated project layer.
        deduplication: ProjectLayerDeduplication,
    },
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
            Self::Appended {
                path,
                deduplication,
            } => {
                if let Some(counts) = deduplication {
                    counts.emit();
                }
                debug_optional_config_path_from_fields("appending project-scope layers", path);
            }
            Self::Deduplicated {
                path,
                deduplication,
            } => {
                deduplication.emit();
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
    collect_file_layers_with_normalizer_and_trace(directory, normalizer, Arc::new(test_env))
        .1
        .map(|layers| layers.into_iter().map(|scoped| scoped.layer).collect())
}

/// Build the discovery chain and project-scope trace using `normalizer`.
pub(super) fn collect_file_layers_with_normalizer_and_trace(
    directory: Option<&Path>,
    normalizer: &impl PathNormalizer,
    env_source: SharedEnvSource,
) -> (Option<ProjectScopeTrace>, OrthoResult<Vec<ScopedFileLayer>>) {
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
    let project_index = project_key.as_deref().and_then(|key| {
        file_layers.value.iter().position(|layer| {
            layer.path().is_some_and(|path| {
                key.as_os_str() == path.as_std_path().as_os_str()
                    || lossy_project_key
                        .as_deref()
                        .is_some_and(|lossy_key| lossy_key == path.as_str())
            })
        })
    });
    let project_trace_path = BoundedConfigPath::from_path(project_file.as_deref());
    if let Some(index) = project_index {
        return (
            Some(ProjectScopeTrace::Included(project_trace_path)),
            Ok(scope_primary_project_layer(file_layers.value, index)),
        );
    }

    merge_project_scope_layers(
        file_layers.value,
        project_file.as_deref(),
        project_trace_path,
    )
}

/// Load project layers and preserve authority when operator and project paths overlap.
fn merge_project_scope_layers(
    discovered_layers: Vec<MergeLayer<'static>>,
    project_file: Option<&Path>,
    project_trace_path: BoundedConfigPath,
) -> (Option<ProjectScopeTrace>, OrthoResult<Vec<ScopedFileLayer>>) {
    let error_trace = ProjectScopeTrace::Appended {
        path: project_trace_path.clone(),
        deduplication: None,
    };
    let result = project_scope_layers(project_file).map(|project_layers| {
        let discovered_layer_count = discovered_layers.len();
        let project_layer_count = project_layers.len();
        let project_index = project_layers
            .last()
            .and_then(MergeLayer::path)
            .and_then(|project| {
                discovered_layers
                    .iter()
                    .position(|layer| layer.path() == Some(project))
            });
        // A shared file reached by both roots has two authorities. Retain both
        // occurrences so project quarantine cannot consume an operator grant,
        // and an operator occurrence cannot suppress a project restriction.
        let appended_layer_count = if project_index.is_some() {
            0
        } else {
            project_layer_count
        };
        let deduplication = ProjectLayerDeduplication::new(
            discovered_layer_count,
            project_layer_count,
            appended_layer_count,
        );
        let trace = if project_layer_count == 0 {
            None
        } else if appended_layer_count == 0 {
            Some(ProjectScopeTrace::Deduplicated {
                path: project_trace_path,
                deduplication,
            })
        } else {
            Some(ProjectScopeTrace::Appended {
                path: project_trace_path,
                deduplication: Some(deduplication),
            })
        };
        let layers = if let Some(index) = project_index {
            scope_primary_project_layer(discovered_layers, index)
        } else {
            discovered_layers
                .into_iter()
                .map(ScopedFileLayer::operator)
                .chain(scope_primary_project_layer(
                    project_layers,
                    project_layer_count.saturating_sub(1),
                ))
                .collect()
        };
        (trace, layers)
    });
    match result {
        Ok((trace, layers)) => (trace, Ok(layers)),
        Err(err) => (Some(error_trace), Err(err)),
    }
}
/// Load the project-scope layers rooted at `project_file`, if one was found.
///
/// # Errors
///
/// Returns an error when the project file cannot be loaded.
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
