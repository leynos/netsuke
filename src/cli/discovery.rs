//! Configuration file discovery and loading helpers.
//!
//! This module locates `OrthoConfig` file layers by scanning for config files
//! through [`ConfigDiscovery`], handling explicit paths from CLI flags and
//! environment variables, and loading TOML chains into [`MergeLayer`] values.

use ortho_config::{
    ConfigDiscovery, MergeComposer, MergeLayer, OrthoResult, load_config_file_as_chain,
};
use std::borrow::Cow;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, debug_span, trace, warn};

use super::parser::Cli;

const CONFIG_ENV_VAR: &str = "NETSUKE_CONFIG";
const CONFIG_ENV_VAR_LEGACY: &str = "NETSUKE_CONFIG_PATH";

/// Classifies an explicit configuration load failure without retaining error text.
///
/// For example, an absent file is [`Self::Missing`], while invalid TOML is
/// [`Self::LoadError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigLoadFailureKind {
    /// The selected configuration file does not exist.
    Missing,
    /// The selected file exists but could not be loaded or parsed.
    LoadError,
}
pub(crate) fn push_file_layers(
    cli: &Cli,
    composer: &mut MergeComposer,
    errors: &mut Vec<Arc<ortho_config::OrthoError>>,
) {
    let resolution = explicit_config_path(cli);
    trace_config_path_resolution(&resolution);
    let layers_result = resolution.path.map_or_else(
        || {
            debug!("using config discovery");
            collect_file_layers(cli.directory.as_deref())
        },
        |path| {
            debug_config_path("using explicit config path", &path);
            load_layers_from_path(&path)
        },
    );
    match layers_result {
        Ok(layers) => {
            for layer in layers {
                composer.push_layer(layer);
            }
        }
        Err(err) => errors.push(err),
    }
}

fn config_discovery(directory: Option<&PathBuf>) -> ConfigDiscovery {
    let mut builder = ConfigDiscovery::builder("netsuke").env_var(CONFIG_ENV_VAR_LEGACY);
    if let Some(dir) = directory {
        builder = builder.clear_project_roots().add_project_root(dir);
    }
    builder.build()
}

pub(crate) fn collect_file_layers(
    directory: Option<&Path>,
) -> OrthoResult<Vec<MergeLayer<'static>>> {
    let discovery = config_discovery(directory.map(PathBuf::from).as_ref());
    let mut file_layers = discovery.compose_layers();
    let mut errors = file_layers.required_errors;
    if file_layers.value.is_empty() {
        errors.append(&mut file_layers.optional_errors);
    }
    if let Some(err) = errors.into_iter().next() {
        return Err(err);
    }

    let project_file = project_scope_file_str(directory);
    let has_project_layer = file_layers.value.iter().any(|layer| {
        layer
            .path()
            .is_some_and(|path| project_file.as_deref() == Some(path.as_str()))
    });
    if has_project_layer {
        debug_optional_config_path(
            "discovery included project-scope layers",
            project_file.as_deref(),
        );
        return Ok(file_layers.value);
    }

    debug_optional_config_path("appending project-scope layers", project_file.as_deref());
    let project_layers = project_scope_layers(directory)?;
    Ok(file_layers
        .value
        .into_iter()
        .chain(project_layers)
        .collect())
}

fn project_scope_file_str(directory: Option<&Path>) -> Option<String> {
    let root = directory
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())?;
    root.join(".netsuke.toml").to_str().map(String::from)
}

fn project_scope_layers(directory: Option<&Path>) -> OrthoResult<Vec<MergeLayer<'static>>> {
    let root = directory
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok());
    let Some(project_file) = root.map(|dir| dir.join(".netsuke.toml")) else {
        return Ok(Vec::new());
    };
    match load_config_file_as_chain(&project_file) {
        Ok(Some(chain)) => Ok(chain
            .values
            .into_iter()
            .map(|(value, path)| MergeLayer::file(Cow::Owned(value), Some(path)))
            .collect()),
        Ok(None) => Ok(Vec::new()),
        Err(err) => Err(err),
    }
}

/// Describes the result of the pure explicit-path selection query.
///
/// The value records the winning selector, its optional path, and each
/// environment lookup needed to make the decision. For example, a present
/// `NETSUKE_CONFIG` wins before `NETSUKE_CONFIG_PATH`. Consumers may emit
/// diagnostics later without giving the query tracing side effects.
#[derive(Debug, PartialEq, Eq)]
struct ConfigPathResolution {
    selector: &'static str,
    path: Option<PathBuf>,
    environment_lookups: Vec<(&'static str, Option<PathBuf>)>,
}
/// Resolve the explicit configuration path requested by `cli`.
///
/// Returns the selector decision and its environment lookup context without
/// emitting events. For example, `cli.config` produces the `cli_flag` selector.
fn explicit_config_path(cli: &Cli) -> ConfigPathResolution {
    resolve_config_selector(cli.config.clone())
}

/// Emit bounded diagnostics for a completed path `resolution`.
///
/// Environment lookups are emitted before the selector event. For example, a
/// selected path contributes a correlation hash and file name, never its full
/// value.
fn trace_config_path_resolution(resolution: &ConfigPathResolution) {
    for (var_name, path) in &resolution.environment_lookups {
        trace_config_path_variable(var_name, path.as_deref());
    }
    debug!(
        selector = resolution.selector,
        path_hash = resolution
            .path
            .as_deref()
            .map(|path| short_hash(path.to_string_lossy().as_bytes()))
            .as_deref(),
        path_file_name = ?resolution.path.as_deref().and_then(Path::file_name),
        path_present = resolution.path.is_some(),
        "resolved config path"
    );
}
/// Select a config path using CLI, primary environment, then legacy environment.
///
/// `cli_config` wins when present; otherwise `NETSUKE_CONFIG` precedes
/// `NETSUKE_CONFIG_PATH`. The returned value includes only the environment
/// lookups actually evaluated, so a CLI selection records none.
fn resolve_config_selector(cli_config: Option<PathBuf>) -> ConfigPathResolution {
    if let Some(path) = cli_config {
        return ConfigPathResolution {
            selector: "cli_flag",
            path: Some(path),
            environment_lookups: Vec::new(),
        };
    }

    let primary_path = env_config_path(CONFIG_ENV_VAR);
    let mut environment_lookups = vec![(CONFIG_ENV_VAR, primary_path.clone())];
    if primary_path.is_some() {
        return ConfigPathResolution {
            selector: CONFIG_ENV_VAR,
            path: primary_path,
            environment_lookups,
        };
    }

    let legacy_path = env_config_path(CONFIG_ENV_VAR_LEGACY);
    environment_lookups.push((CONFIG_ENV_VAR_LEGACY, legacy_path.clone()));
    ConfigPathResolution {
        selector: legacy_path
            .as_ref()
            .map_or("none", |_| CONFIG_ENV_VAR_LEGACY),
        path: legacy_path,
        environment_lookups,
    }
}
/// Read a non-empty path from environment variable `var_name`.
///
/// Returns `None` when the variable is missing or empty. For example, an empty
/// `NETSUKE_CONFIG` does not prevent the legacy variable from being queried.
/// This query does not emit tracing events.
fn env_config_path(var_name: &str) -> Option<PathBuf> {
    std::env::var_os(var_name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Trace one environment lookup using bounded path fields.
///
/// `var_name` and presence are recorded directly. A present `path` contributes
/// only its file name and correlation hash; the full path is never logged.
fn trace_config_path_variable(var_name: &str, path: Option<&Path>) {
    trace!(
        var_name,
        found = path.is_some(),
        path_hash = path
            .map(|value| short_hash(value.to_string_lossy().as_bytes()))
            .as_deref(),
        path_file_name = ?path.and_then(Path::file_name),
        "read config path variable"
    );
}
pub(crate) fn load_layers_from_path(path: &Path) -> OrthoResult<Vec<MergeLayer<'static>>> {
    match load_config_file_as_chain(path) {
        Ok(Some(chain)) => Ok(chain
            .values
            .into_iter()
            .map(|(value, layer_path)| MergeLayer::file(Cow::Owned(value), Some(layer_path)))
            .collect()),
        Ok(None) => {
            let error = Arc::new(ortho_config::OrthoError::File {
                path: path.to_path_buf(),
                source: Box::new(io::Error::new(
                    io::ErrorKind::NotFound,
                    "explicit configuration file not found",
                )),
            });
            warn_explicit_config_load_failed(path, ConfigLoadFailureKind::Missing);
            Err(error)
        }
        Err(error) => {
            warn_explicit_config_load_failed(path, ConfigLoadFailureKind::LoadError);
            Err(error)
        }
    }
}

/// Warn that an explicit `path` failed with `failure_kind`.
///
/// The event exposes the failure class, file name, and correlation hash, but
/// neither the full path nor the formatted parser or I/O error.
fn warn_explicit_config_load_failed(path: &Path, failure_kind: ConfigLoadFailureKind) {
    warn!(
        path_hash = %short_hash(path.to_string_lossy().as_bytes()),
        path_file_name = ?path.file_name(),
        failure_kind = ?failure_kind,
        "explicit config load failed"
    );
}

/// Emit `message` with bounded fields identifying `path`.
///
/// For example, an explicit config branch records a file name and correlation
/// hash rather than the full path.
fn debug_config_path(message: &'static str, path: &Path) {
    debug!(
        path_hash = %short_hash(path.to_string_lossy().as_bytes()),
        path_file_name = ?path.file_name(),
        message
    );
}

/// Emit `message` with presence and bounded fields for an optional path string.
///
/// `None` records only absence. A present value records its file name and a
/// correlation hash, never the complete path.
fn debug_optional_config_path(message: &'static str, path: Option<&str>) {
    debug!(
        path_hash = path.map(|value| short_hash(value.as_bytes())).as_deref(),
        path_file_name = ?path.and_then(|value| Path::new(value).file_name()),
        path_present = path.is_some(),
        message
    );
}
/// Return a stable-width correlation identifier for `value`.
///
/// For example, the same path bytes produce the same 16-character identifier
/// within the supported runtime. This hash bounds log cardinality; it is not a
/// cryptographic digest and must not be used as a security boundary.
fn short_hash(value: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
pub(crate) fn collect_diag_file_layers(cli: &Cli) -> OrthoResult<Vec<MergeLayer<'static>>> {
    let _span = debug_span!("collect_diag_file_layers").entered();

    let resolution = explicit_config_path(cli);
    trace_config_path_resolution(&resolution);
    resolution.path.map_or_else(
        || {
            debug!("using config discovery");
            collect_file_layers(cli.directory.as_deref())
        },
        |path| {
            debug_config_path("using explicit config path", &path);
            load_layers_from_path(&path)
        },
    )
}

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod tests;

/// Tests for the explicit config-path selector precedence implemented by
/// [`explicit_config_path`]. Enumerated cases cover all 2^3 combinations of
/// `--config`, `NETSUKE_CONFIG`, and `NETSUKE_CONFIG_PATH` presence; a proptest
/// property test asserts the invariant for generated path values.
#[cfg(test)]
#[path = "config_path_precedence_tests.rs"]
mod config_path_precedence_tests;
