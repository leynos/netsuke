//! Configuration file discovery and loading helpers.
//!
//! This module locates `OrthoConfig` file layers by scanning for config files
//! through [`ConfigDiscovery`], handling explicit paths from CLI flags and
//! environment variables, and loading TOML chains into [`MergeLayer`] values.

use ortho_config::{
    MapEnv, MergeComposer, MergeLayer, OrthoResult, SharedEnvSource, load_config_file_as_chain,
};
use std::borrow::Cow;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::parser::Cli;

#[path = "discovery_diagnostics.rs"]
mod diagnostics;
#[path = "discovery_layers.rs"]
mod layers;
#[path = "discovery_paths.rs"]
mod paths;
#[path = "discovery_trace.rs"]
mod trace;
use diagnostics::{BoundedConfigPath, ConfigLoadFailureKind, ConfigLoadWarning};
use layers::collect_file_layers_with_trace_and_env_source;
use trace::{DiscoveryDiagnostics, DiscoveryTrace, FileLayerTrace};

const CONFIG_ENV_VAR: &str = "NETSUKE_CONFIG";
const DISCOVERY_ENV_KEYS: [&str; 7] = [
    CONFIG_ENV_VAR,
    "HOME",
    "USERPROFILE",
    "XDG_CONFIG_HOME",
    "XDG_CONFIG_DIRS",
    "APPDATA",
    "LOCALAPPDATA",
];

/// Provides access to environment variables used during config discovery.
///
/// Production code uses [`StdEnvProvider`]. Tests can provide an in-memory
/// implementation so config-selection logic does not mutate process-global
/// environment state.
pub trait EnvProvider {
    /// Return the value of `key`, or `None` when the key is unset.
    fn get(&self, key: &str) -> Option<OsString>;

    /// Return all values available to the configuration environment layer.
    ///
    /// Providers concerned only with selector lookup may retain the empty
    /// default. Full merge providers override this method.
    fn entries(&self) -> Vec<(OsString, OsString)> {
        Vec::new()
    }
}

/// Environment provider backed by [`std::env::var_os`].
#[derive(Debug, Default, Clone, Copy)]
pub struct StdEnvProvider;

#[expect(
    clippy::disallowed_methods,
    reason = "composition root: StdEnvProvider is the process-backed adapter behind the EnvProvider seam"
)]
impl EnvProvider for StdEnvProvider {
    fn get(&self, key: &str) -> Option<OsString> {
        std::env::var_os(key)
    }

    fn entries(&self) -> Vec<(OsString, OsString)> {
        std::env::vars_os().collect()
    }
}

/// File layers and loading errors produced by one discovery pass.
///
/// The diagnostic pre-pass borrows the layers to resolve JSON output, then the
/// full merge consumes the same result. Keeping errors beside the layers lets
/// those phases retain their distinct error policies without rediscovery.
pub struct DiscoveredLayers {
    layers: Vec<MergeLayer<'static>>,
    json_preference: bool,
    errors: Vec<Arc<ortho_config::OrthoError>>,
    diagnostics: DiscoveryDiagnostics,
}

impl DiscoveredLayers {
    /// Borrow the file layers in discovery order.
    #[cfg(test)]
    pub(crate) fn layers(&self) -> &[MergeLayer<'static>] {
        &self.layers
    }

    /// Borrow the first discovery error, if loading failed.
    pub(crate) fn first_error(&self) -> Option<&Arc<ortho_config::OrthoError>> {
        self.errors.first()
    }

    /// Return the last valid JSON preference from the discovered file layers.
    pub(crate) const fn json_preference(&self) -> bool {
        self.json_preference
    }

    pub(crate) fn into_parts(
        self,
    ) -> (Vec<MergeLayer<'static>>, Vec<Arc<ortho_config::OrthoError>>) {
        (self.layers, self.errors)
    }
}

/// Layers and diagnostics returned by a side-effect-free discovery pass.
///
/// The diagnostic pre-pass reads the layers while retaining the bounded events
/// for a composition boundary to emit after it installs the tracing filter.
pub struct DiscoveryOutcome {
    layers: DiscoveredLayers,
}

impl DiscoveryOutcome {
    /// Borrow file layers in discovery order.
    #[cfg(test)]
    pub(crate) fn layers(&self) -> &[MergeLayer<'static>] {
        self.layers.layers()
    }

    /// Borrow the first discovery error, if loading failed.
    pub(crate) fn first_error(&self) -> Option<&Arc<ortho_config::OrthoError>> {
        self.layers.first_error()
    }

    pub(crate) const fn json_preference(&self) -> bool {
        self.layers.json_preference()
    }

    /// Consume the outcome into the reusable file layers.
    #[must_use]
    pub fn into_layers(self) -> DiscoveredLayers {
        self.layers
    }

    /// Emit deferred diagnostics without repeating discovery.
    pub fn emit_diagnostics(&self) {
        self.layers.diagnostics.emit();
    }
}

/// Discover configuration layers once through the injected environment.
pub(crate) fn discover_file_layers(cli: &Cli, env: &impl EnvProvider) -> DiscoveryOutcome {
    let (trace, load_warning, outcome) = collect_file_layers_with_env(cli, env);
    let diagnostics = DiscoveryDiagnostics::new(trace, load_warning);
    let layers = match outcome {
        Ok(discovered_layers) => {
            let (layers, json_preference) =
                layers::retain_layers_and_resolve_json(discovered_layers);
            DiscoveredLayers {
                layers,
                json_preference,
                errors: Vec::new(),
                diagnostics,
            }
        }
        Err(error) => DiscoveredLayers {
            layers: Vec::new(),
            json_preference: Cli::default().json,
            errors: vec![error],
            diagnostics,
        },
    };
    DiscoveryOutcome { layers }
}

/// Add a discovered file layer result to a merge composition.
///
/// Discovery errors join the merge error collection, retaining the normal
/// merge path's accumulated-error behaviour.
pub(crate) fn push_discovered_file_layers(
    composer: &mut MergeComposer,
    errors: &mut Vec<Arc<ortho_config::OrthoError>>,
    discovered: DiscoveredLayers,
) {
    let (layers, discovery_errors) = discovered.into_parts();
    errors.extend(discovery_errors);
    for layer in layers {
        composer.push_layer(layer);
    }
}

/// Load layers through the shared explicit-config precedence boundary.
///
/// Normal merging and early JSON resolution both use this helper so they
/// select the same file layers while retaining their own error handling.
fn collect_file_layers_with_env(
    cli: &Cli,
    env: &impl EnvProvider,
) -> (
    DiscoveryTrace,
    Option<ConfigLoadWarning>,
    OrthoResult<Vec<MergeLayer<'static>>>,
) {
    let resolution = resolve_config_selector(cli.config.clone(), env);
    let (file_layers, load_warning, outcome) = resolution.path.as_deref().map_or_else(
        || {
            let (project_scope, outcome) = collect_file_layers_with_trace_and_env_source(
                cli.directory.as_deref(),
                discovery_env_source(env),
            );
            (FileLayerTrace::Automatic { project_scope }, None, outcome)
        },
        |path| {
            // `-C` behaves as a working-directory change for CLI paths,
            // including an explicit configuration selector.
            let effective_path = cli
                .directory
                .as_deref()
                .filter(|_| path.is_relative())
                .map_or_else(|| path.to_path_buf(), |directory| directory.join(path));
            let (load_warning, outcome) = load_layers_from_path_with_warning(&effective_path);
            (
                FileLayerTrace::Explicit {
                    path: BoundedConfigPath::from_path(Some(&effective_path)),
                },
                load_warning,
                outcome,
            )
        },
    );
    (
        DiscoveryTrace::new(&resolution, file_layers),
        load_warning,
        outcome,
    )
}

/// Project the fixed discovery inputs from Netsuke's environment port.
///
/// This adapter is private to CLI configuration composition. It intentionally
/// exposes only discovery's documented lookup keys: `EnvironmentLayer` remains
/// the sole owner of complete `NETSUKE_*` enumeration for value merging.
pub(crate) fn discovery_env_source(env: &impl EnvProvider) -> SharedEnvSource {
    let mut source = MapEnv::new();
    for key in DISCOVERY_ENV_KEYS {
        if let Some(value) = env.get(key) {
            source.insert(key, value);
        }
    }
    Arc::new(source)
}

/// Select an explicit config path, giving `--config` precedence over `env`.
///
/// A thin wrapper over [`resolve_config_selector`] for callers that need only
/// the winning path. Like that query it performs no tracing; discovery returns
/// bounded diagnostics for composition boundaries to emit later.
///
/// Production code takes the richer [`ConfigPathResolution`] so it can trace the
/// environment lookups, leaving this as a convenience for precedence tests.
#[cfg(test)]
pub(crate) fn explicit_config_path_with_env(cli: &Cli, env: &impl EnvProvider) -> Option<PathBuf> {
    resolve_config_selector(cli.config.clone(), env).path
}

/// Describes the result of the pure explicit-path selection query.
///
/// Records the winning selector, its optional path, and every environment
/// lookup evaluated to reach the decision, so a caller can emit diagnostics
/// afterwards without giving the query tracing side effects.
#[derive(Debug, PartialEq, Eq)]
struct ConfigPathResolution {
    selector: &'static str,
    path: Option<PathBuf>,
    environment_lookups: Vec<(&'static str, Option<PathBuf>)>,
}

/// Select a config path from the CLI flag, then `NETSUKE_CONFIG` via `env`.
///
/// `cli_config` wins when present, in which case no environment lookup is
/// recorded because none is performed. This query emits no tracing.
fn resolve_config_selector(
    cli_config: Option<PathBuf>,
    env: &impl EnvProvider,
) -> ConfigPathResolution {
    if let Some(path) = cli_config {
        return ConfigPathResolution {
            selector: "cli_flag",
            path: Some(path),
            environment_lookups: Vec::new(),
        };
    }

    let primary_path = env_config_path(env, CONFIG_ENV_VAR);
    ConfigPathResolution {
        selector: primary_path.as_ref().map_or("none", |_| CONFIG_ENV_VAR),
        environment_lookups: vec![(CONFIG_ENV_VAR, primary_path.clone())],
        path: primary_path,
    }
}
/// Read a non-empty config path from `var_name` through `env`.
///
/// Returns `None` when the variable is unset or empty, so discovery still runs.
/// This query emits no tracing.
fn env_config_path(env: &impl EnvProvider, var_name: &str) -> Option<PathBuf> {
    env.get(var_name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Load the configuration chain rooted at an explicit file path.
///
/// Unlike discovery, a missing explicit file is an error because the caller
/// selected it deliberately.
#[cfg(test)]
pub(crate) fn load_layers_from_path(path: &Path) -> OrthoResult<Vec<MergeLayer<'static>>> {
    let (load_warning, result) = load_layers_from_path_with_warning(path);
    if let Some(warning) = load_warning {
        warning.emit();
    }
    result
}

/// Load explicit layers while retaining a warning for the composition boundary.
fn load_layers_from_path_with_warning(
    path: &Path,
) -> (
    Option<ConfigLoadWarning>,
    OrthoResult<Vec<MergeLayer<'static>>>,
) {
    match load_config_file_as_chain(path) {
        Ok(Some(chain)) => (
            None,
            Ok(chain
                .values
                .into_iter()
                .map(|(value, layer_path)| MergeLayer::file(Cow::Owned(value), Some(layer_path)))
                .collect()),
        ),
        Ok(None) => {
            let error = Arc::new(ortho_config::OrthoError::File {
                path: path.to_path_buf(),
                source: Box::new(io::Error::new(
                    io::ErrorKind::NotFound,
                    "explicit configuration file not found",
                )),
            });
            (
                Some(ConfigLoadWarning::new(path, ConfigLoadFailureKind::Missing)),
                Err(error),
            )
        }
        Err(error) => (
            Some(ConfigLoadWarning::new(
                path,
                ConfigLoadFailureKind::LoadError,
            )),
            Err(error),
        ),
    }
}

/// Load file layers for early JSON resolution using injected environment access.
///
/// This delegates to the same precedence boundary as the normal merge path.
pub(crate) fn collect_diag_file_layers_with_env(
    cli: &Cli,
    env: &impl EnvProvider,
) -> DiscoveryOutcome {
    discover_file_layers(cli, env)
}

#[cfg(test)]
#[path = "discovery_event_assertions.rs"]
mod event_assertions;

#[cfg(test)]
#[path = "discovery_tracing_tests.rs"]
mod tracing_tests;

#[cfg(test)]
#[path = "discovery_layer_tests.rs"]
mod layer_tests;

#[cfg(test)]
#[path = "discovery_helper_proptests.rs"]
mod helper_proptests;

#[cfg(test)]
#[path = "discovery_replay_proptests.rs"]
mod replay_proptests;

/// Tests for explicit config-path precedence. Enumerated cases cover every
/// combination of `--config` and `NETSUKE_CONFIG` presence; a proptest property
/// test asserts the invariant for generated path values.
#[cfg(test)]
#[path = "config_path_precedence_tests.rs"]
mod config_path_precedence_tests;
#[cfg(test)]
#[path = "discovery_unit_tests.rs"]
mod unit_tests;
