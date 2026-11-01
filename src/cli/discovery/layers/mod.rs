//! Discover configuration layers while preserving each root's authority.
//!
//! Runs the `OrthoConfig` scan and applies the project-scope second pass, so a
//! project `.netsuke.toml` outranks user-scope files. Project-chain occurrences
//! retain project authority through quarantine; the same file reached from an
//! operator root keeps operator authority. The project file's comparison key
//! and its fallback policy live in [`project_key`] because that policy is a
//! discovery decision.

#[cfg(test)]
use ortho_config::MapEnv;
use ortho_config::{
    ConfigDiscovery, FileLayerChain, MergeLayer, OrthoResult, SharedEnvSource,
    load_config_file_as_chain,
};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::Arc;

use super::diagnostics::{
    BoundedConfigPath, ProjectLayerDeduplication, debug_optional_config_path_from_fields,
};
use super::paths::{PathNormalizer, project_scope_file};
use super::project_policy::scope_project_chain;

mod project_key;
use project_key::ProjectKey;

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

/// Authority evidence for one discovered root.
///
/// Groups the two facts every occurrence is classified against, so the merge
/// helpers stay readable and their parameter lists stay within the lint limit.
#[derive(Clone, Copy)]
struct RootTrust<'a> {
    /// Whether the root that produced the chain is an operator root.
    is_trusted_root: bool,
    /// Classified identity of the project configuration file, when one exists.
    project_key: Option<&'a ProjectKey>,
}

/// Project-scope inputs for the second discovery pass.
struct ProjectScope<'a> {
    /// Project configuration path used for the second pass.
    file: Option<&'a Path>,
    /// Bounded path metadata replayed for the project-scope trace.
    trace_path: BoundedConfigPath,
    /// Authority evidence for the discovered root.
    trust: RootTrust<'a>,
}

/// Build the restricted automatic discovery scanner.
///
/// Anchors the project root to `directory` when supplied; otherwise the default
/// project roots apply. The builder is deliberately restricted to location
/// candidates: it registers no selector environment variable and injects no
/// explicit path, so every candidate is an operator-location or project-root
/// candidate and the walk cannot be steered from the environment or CLI.
fn config_discovery(directory: Option<&PathBuf>, env_source: SharedEnvSource) -> ConfigDiscovery {
    let mut builder = ConfigDiscovery::builder("netsuke").env_source(env_source);
    if let Some(dir) = directory {
        builder = builder.clear_project_roots().add_project_root(dir);
    }
    builder.build()
}

/// Return the first automatically discovered chain and whether its root is trusted.
///
/// Mirrors `OrthoConfig`'s first-success traversal: candidates are tried in
/// order, a missing candidate continues to the next one, and the first chain
/// found wins. Load errors are retained only for the no-chain case, matching
/// the optional-error handling the previous single call produced.
fn first_automatic_chain(
    discovery: &ConfigDiscovery,
    project_key: Option<&ProjectKey>,
) -> OrthoResult<(Vec<MergeLayer<'static>>, bool)> {
    let mut optional_errors = Vec::new();
    for candidate in discovery.candidates() {
        match load_config_file_as_chain(&candidate) {
            Ok(Some(chain)) => {
                let is_trusted = !project_key
                    .is_some_and(|key| key.is_ambiguous() || key.matches_path(&candidate));
                return Ok((chain_layers(chain), is_trusted));
            }
            Ok(None) => {}
            Err(error) => optional_errors.push(error),
        }
    }
    optional_errors
        .into_iter()
        .next()
        .map_or_else(|| Ok((Vec::new(), true)), Err)
}

/// Flatten a file layer chain into owned merge layers.
fn chain_layers(chain: FileLayerChain) -> Vec<MergeLayer<'static>> {
    chain
        .values
        .into_iter()
        .map(|(value, path)| MergeLayer::file(Cow::Owned(value), Some(path)))
        .collect()
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
    let project_file = project_scope_file(directory);
    let project_key = project_file
        .as_deref()
        .map(|path| ProjectKey::classify(normalizer, path));
    let discovery = config_discovery(directory.map(PathBuf::from).as_ref(), env_source);
    let (file_layers, is_trusted_root) =
        match first_automatic_chain(&discovery, project_key.as_ref()) {
            Ok(found) => found,
            Err(error) => return (None, Err(error)),
        };

    let project_index = project_key.as_ref().and_then(|key| {
        file_layers
            .iter()
            .position(|layer| layer.path().is_some_and(|path| key.matches(path)))
    });
    let project_trace_path = BoundedConfigPath::from_path(project_file.as_deref());
    if let Some(index) = project_index.filter(|index| *index + 1 == file_layers.len()) {
        return (
            Some(ProjectScopeTrace::Included(project_trace_path)),
            Ok(scope_project_chain(file_layers, index)),
        );
    }

    merge_project_scope_layers(
        file_layers,
        ProjectScope {
            file: project_file.as_deref(),
            trace_path: project_trace_path,
            trust: RootTrust {
                is_trusted_root,
                project_key: project_key.as_ref(),
            },
        },
    )
}

/// Load project layers and preserve authority when operator and project paths overlap.
fn merge_project_scope_layers(
    discovered_layers: Vec<MergeLayer<'static>>,
    scope: ProjectScope<'_>,
) -> (Option<ProjectScopeTrace>, OrthoResult<Vec<ScopedFileLayer>>) {
    let ProjectScope {
        file: project_file,
        trace_path: project_trace_path,
        trust,
    } = scope;
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
        // occurrences so project-chain quarantine cannot consume an operator
        // grant, and an operator occurrence cannot suppress a project request.
        let project_is_root =
            project_index.is_some_and(|index| index + 1 == discovered_layer_count);
        let appended_layer_count = if project_is_root {
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
        let layers = scope_merged_layers(discovered_layers, project_layers, project_index, trust);
        (trace, layers)
    });
    match result {
        Ok((trace, layers)) => (trace, Ok(layers)),
        Err(err) => (Some(error_trace), Err(err)),
    }
}

/// Assign authority per root occurrence without discarding shared files.
///
/// The operator root keeps its occurrences operator-scoped, except an occurrence
/// that is the project configuration file: a project can influence that file
/// wherever it is read from, so its authority is withheld. The separately loaded
/// project chain scopes its inherited parents through the project file and never
/// grants definition authority. When the discovered root itself is the project
/// file, the whole chain descends from a project-controlled root and is
/// distrusted as a unit.
fn scope_merged_layers(
    discovered_layers: Vec<MergeLayer<'static>>,
    project_layers: Vec<MergeLayer<'static>>,
    project_index: Option<usize>,
    trust: RootTrust<'_>,
) -> Vec<ScopedFileLayer> {
    let project_is_root = project_index.is_some_and(|index| index + 1 == discovered_layers.len());
    if let Some(index) = project_index.filter(|_| project_is_root) {
        return scope_project_chain(discovered_layers, index);
    }
    let primary_index = project_layers.len().saturating_sub(1);
    discovered_layers
        .into_iter()
        .map(|layer| scope_discovered_occurrence(layer, trust.is_trusted_root, trust.project_key))
        .chain(scope_project_chain(project_layers, primary_index))
        .collect()
}

/// Tag one operator-root occurrence with the authority its provenance earns.
///
/// A synthetic layer with no recorded path has unknown provenance, so it is
/// treated as untrusted rather than assumed to be operator configuration.
fn scope_discovered_occurrence(
    layer: MergeLayer<'static>,
    is_trusted_root: bool,
    project_key: Option<&ProjectKey>,
) -> ScopedFileLayer {
    let is_project_occurrence = match (project_key, layer.path()) {
        (Some(key), Some(path)) => key.is_ambiguous() || key.matches(path),
        (None, _) => false,
        (Some(_), None) => true,
    };
    if is_trusted_root && !is_project_occurrence {
        ScopedFileLayer::operator(layer)
    } else {
        ScopedFileLayer::untrusted(layer)
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

#[cfg(test)]
mod property_tests;
#[cfg(test)]
mod quarantine_trace_tests;
#[cfg(test)]
mod replay_tests;
#[cfg(test)]
mod selector_tests;
#[cfg(test)]
mod tests;
