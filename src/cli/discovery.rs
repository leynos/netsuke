//! Configuration file discovery and loading helpers.
//!
//! This module locates `OrthoConfig` file layers by scanning for config files
//! through [`ConfigDiscovery`], handling explicit paths from CLI flags and
//! environment variables, and loading TOML chains into [`MergeLayer`] values.
use ortho_config::{MapEnv, MergeLayer, OrthoResult, SharedEnvSource, load_config_file_as_chain};
use std::borrow::Cow;
use std::io;
use std::path::Path;
use std::sync::Arc;

use super::command::Cli;
use crate::host_pattern::HostPattern;

#[path = "discovery_environment.rs"]
mod environment;
pub use environment::{EnvProvider, StdEnvProvider};

#[path = "discovery_diagnostics.rs"]
mod diagnostics;
#[path = "discovery_json.rs"]
mod json;
#[path = "discovery_layers.rs"]
mod layers;
#[path = "discovery_paths.rs"]
mod paths;
#[path = "discovery_project_policy.rs"]
mod project_policy;

#[path = "discovery_selector.rs"]
mod selector;
#[path = "discovery_trace.rs"]
mod trace;

#[path = "discovery_telemetry.rs"]
mod telemetry;
use diagnostics::{BoundedConfigPath, ConfigLoadFailureKind, ConfigLoadWarning};
use layers::collect_file_layers_with_normalizer_and_trace;
use paths::{FsPathNormalizer, PathNormalizer};
#[cfg(test)]
use selector::{
    ConfigPathResolution, env_config_path, explicit_config_path_with_env, resolve_config_selector,
};
#[cfg(test)]
use std::path::PathBuf;
/// Record the discovery series for an already-timed phase at the boundary.
pub use telemetry::record_discovery_outcome;
use trace::{DiscoveryDiagnostics, DiscoveryTrace, FileLayerTrace};

#[path = "discovery_merge_layers.rs"]
mod merge_layers;
pub(crate) use merge_layers::push_discovered_file_layers;
/// Name of the environment variable that selects the configuration file.
///
/// Read as the primary selector after the `--config` CLI flag when a path is
/// not given explicitly.
const CONFIG_ENV_VAR: &str = "NETSUKE_CONFIG";
/// Environment variables consulted while discovering configuration layers.
const DISCOVERY_ENV_KEYS: [&str; 7] = [
    CONFIG_ENV_VAR,
    "HOME",
    "USERPROFILE",
    "XDG_CONFIG_HOME",
    "XDG_CONFIG_DIRS",
    "APPDATA",
    "LOCALAPPDATA",
];

/// Project-scoped fetch-policy restrictions captured before generic merging.
///
/// Project configuration is untrusted relative to operator configuration. The
/// merge boundary uses this request to preserve restrictions without allowing
/// the project layer to widen operator grants.
#[derive(Debug, Default)]
pub(crate) struct ProjectFetchPolicyRequest {
    /// Optional request to deny every host by default.
    pub(crate) default_deny: Option<bool>,
    /// Additional schemes requested by the project configuration.
    pub(crate) allow_scheme: Vec<String>,
    /// Additional hosts requested by the project configuration.
    pub(crate) allow_host: Vec<HostPattern>,
}

/// File layers and loading errors produced by one discovery pass.
///
/// The diagnostic pre-pass borrows the layers to resolve JSON output, then the
/// full merge consumes the same result. Keeping errors beside the layers lets
/// those phases retain their distinct error policies without rediscovery.
pub struct DiscoveredLayers {
    /// File layers found in discovery order, before precedence resolution.
    layers: Vec<MergeLayer<'static>>,
    /// Whether any discovered layer requested JSON output.
    json_preference: bool,
    /// Fetch-policy restrictions requested by the primary project file.
    project_fetch_policy_request: ProjectFetchPolicyRequest,
    /// Loading errors deferred beside the layers that may still be usable.
    errors: Vec<Arc<ortho_config::OrthoError>>,
    /// Bounded trace for composition boundaries to emit after the merge.
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

    /// Consume into the raw layers and deferred discovery errors.
    pub(crate) fn into_parts(
        self,
    ) -> (
        Vec<MergeLayer<'static>>,
        Vec<Arc<ortho_config::OrthoError>>,
        ProjectFetchPolicyRequest,
    ) {
        (self.layers, self.errors, self.project_fetch_policy_request)
    }
}

/// Layers and diagnostics returned by a side-effect-free discovery pass.
///
/// The diagnostic pre-pass reads the layers while retaining the bounded events
/// for a composition boundary to emit after it installs the tracing filter.
pub struct DiscoveryOutcome {
    /// File layers and deferred loading errors from the discovery pass.
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

    /// Return the last valid JSON preference from the discovered file layers.
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
    discover_file_layers_with_normalizer(cli, env, &FsPathNormalizer)
}

/// Discover configuration layers through one path-normalization policy.
fn discover_file_layers_with_normalizer(
    cli: &Cli,
    env: &impl EnvProvider,
    normalizer: &impl PathNormalizer,
) -> DiscoveryOutcome {
    let (trace, load_warning, outcome) = collect_file_layers_with_env(cli, env, normalizer);
    let diagnostics = DiscoveryDiagnostics::new(trace, load_warning);
    let layers = match outcome {
        Ok(discovered_layers) => match layers::retain_layers_and_resolve_json(
            discovered_layers,
            cli.directory.as_deref().map(camino::Utf8Path::as_std_path),
            normalizer,
        ) {
            Ok((layers, json_preference, project_fetch_policy_request)) => DiscoveredLayers {
                layers,
                json_preference,
                project_fetch_policy_request,
                errors: Vec::new(),
                diagnostics,
            },
            Err(error) => DiscoveredLayers {
                layers: Vec::new(),
                json_preference: Cli::default().json,
                project_fetch_policy_request: ProjectFetchPolicyRequest::default(),
                errors: vec![error],
                diagnostics,
            },
        },
        Err(error) => DiscoveredLayers {
            layers: Vec::new(),
            json_preference: Cli::default().json,
            project_fetch_policy_request: ProjectFetchPolicyRequest::default(),
            errors: vec![error],
            diagnostics,
        },
    };
    DiscoveryOutcome { layers }
}

/// Load layers through the shared explicit-config precedence boundary.
/// Normal merging and early JSON resolution use it to retain their error policy.
fn collect_file_layers_with_env(
    cli: &Cli,
    env: &impl EnvProvider,
    normalizer: &impl PathNormalizer,
) -> (
    DiscoveryTrace,
    Option<ConfigLoadWarning>,
    OrthoResult<Vec<MergeLayer<'static>>>,
) {
    let resolution = selector::resolve_config_selector(cli.config.clone(), env);
    let (file_layers, load_warning, outcome) = resolution.path.as_deref().map_or_else(
        || {
            let (project_scope, outcome) = collect_file_layers_with_normalizer_and_trace(
                cli.directory.as_deref().map(camino::Utf8Path::as_std_path),
                normalizer,
                discovery_env_source(env),
            );
            (FileLayerTrace::Automatic { project_scope }, None, outcome)
        },
        |path| {
            // Explicit selectors are independent of `-C/--directory`.
            // Relative paths retain their selector spelling and resolve
            // against the process working directory at load time; absolute
            // paths remain unchanged.
            let (load_warning, outcome) = load_layers_from_path_with_warning(path);
            (
                FileLayerTrace::Explicit {
                    path: BoundedConfigPath::from_path(Some(path)),
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
#[path = "discovery_helper_proptests.rs"]
mod helper_proptests;

#[cfg(test)]
#[path = "discovery_layer_replay_tests.rs"]
mod layer_replay_tests;
#[cfg(test)]
#[path = "discovery_layer_selector_tests.rs"]
mod layer_selector_tests;
#[cfg(test)]
#[path = "discovery_layer_tests.rs"]
mod layer_tests;
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
