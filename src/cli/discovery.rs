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
use std::ffi::OsString;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, debug_span, trace, warn};

use super::parser::Cli;

const CONFIG_ENV_VAR: &str = "NETSUKE_CONFIG";

/// Provides access to environment variables used during config discovery.
///
/// Production code uses [`StdEnvProvider`]. Tests can provide an in-memory
/// implementation so config-selection logic does not mutate process-global
/// environment state.
pub trait EnvProvider {
    /// Return the value of `key`, or `None` when the key is unset.
    fn get(&self, key: &str) -> Option<OsString>;
}

/// Environment provider backed by [`std::env::var_os`].
#[derive(Debug, Default, Clone, Copy)]
pub struct StdEnvProvider;

impl EnvProvider for StdEnvProvider {
    fn get(&self, key: &str) -> Option<OsString> {
        std::env::var_os(key)
    }
}

pub(crate) fn push_file_layers(
    cli: &Cli,
    composer: &mut MergeComposer,
    errors: &mut Vec<Arc<ortho_config::OrthoError>>,
) {
    push_file_layers_with_env(cli, composer, errors, &StdEnvProvider);
}

/// Load configuration layers with environment access supplied by `env`.
///
/// Loading errors are appended to `errors`, matching the normal merge path
/// without requiring callers to mutate the process environment.
pub(crate) fn push_file_layers_with_env(
    cli: &Cli,
    composer: &mut MergeComposer,
    errors: &mut Vec<Arc<ortho_config::OrthoError>>,
    env: &impl EnvProvider,
) {
    match collect_file_layers_with_env(cli, env) {
        Ok(layers) => {
            for layer in layers {
                composer.push_layer(layer);
            }
        }
        Err(err) => errors.push(err),
    }
}

/// Load layers through the shared explicit-config precedence boundary.
///
/// Normal merging and early JSON resolution both use this helper so they
/// select the same file layers while retaining their own error handling.
fn collect_file_layers_with_env(
    cli: &Cli,
    env: &impl EnvProvider,
) -> OrthoResult<Vec<MergeLayer<'static>>> {
    let resolution = resolve_config_selector(cli.config.clone(), env);
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

fn config_discovery(directory: Option<&PathBuf>) -> ConfigDiscovery {
    let mut builder = ConfigDiscovery::builder("netsuke").env_var(CONFIG_ENV_VAR);
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
    let project_key = project_file.as_deref().map(normalized_path_key);
    let has_project_layer = file_layers.value.iter().any(|layer| {
        layer.path().is_some_and(|path| {
            project_key.as_deref() == Some(normalized_path_key(path.as_str()).as_path())
        })
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
/// Select an explicit config path, giving `--config` precedence over `env`.
///
/// A thin wrapper over [`resolve_config_selector`] for callers that need only
/// the winning path. Like that query it performs no tracing; orchestration
/// boundaries call [`trace_config_path_resolution`] to emit diagnostics.
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

/// Emit bounded diagnostics for a completed path `resolution`.
///
/// Environment lookups are traced before the selector event. A selected path
/// contributes only a correlation hash and file name, never its full value.
fn trace_config_path_resolution(resolution: &ConfigPathResolution) {
    for (var_name, path) in &resolution.environment_lookups {
        trace_config_path_variable(var_name, path.as_deref());
    }
    debug!(
        selector = resolution.selector,
        path_hash = resolution.path.as_deref().map(path_hash).as_deref(),
        path_file_name = ?resolution.path.as_deref().and_then(Path::file_name),
        path_present = resolution.path.is_some(),
        "resolved config path"
    );
}

/// Trace one environment lookup using bounded path fields.
fn trace_config_path_variable(var_name: &str, path: Option<&Path>) {
    trace!(
        var_name,
        found = path.is_some(),
        path_hash = path.map(path_hash).as_deref(),
        path_file_name = ?path.and_then(Path::file_name),
        "read config path variable"
    );
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
pub(crate) fn load_layers_from_path(
    path: &std::path::Path,
) -> OrthoResult<Vec<MergeLayer<'static>>> {
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

/// Load file layers for early JSON resolution using injected environment access.
///
/// This delegates to the same precedence boundary as the normal merge path.
pub(crate) fn collect_diag_file_layers_with_env(
    cli: &Cli,
    env: &impl EnvProvider,
) -> OrthoResult<Vec<MergeLayer<'static>>> {
    let _span = debug_span!("collect_diag_file_layers").entered();
    collect_file_layers_with_env(cli, env)
}

/// Classifies an explicit configuration load failure without retaining error text.
///
/// An absent file is [`Self::Missing`]; invalid TOML is [`Self::LoadError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigLoadFailureKind {
    /// The selected configuration file does not exist.
    Missing,
    /// The selected file exists but could not be loaded or parsed.
    LoadError,
}

/// Warn that an explicit `path` failed with `failure_kind`.
///
/// The event exposes the failure class, file name, and correlation hash, but
/// neither the full path nor the formatted parser or I/O error.
fn warn_explicit_config_load_failed(path: &Path, failure_kind: ConfigLoadFailureKind) {
    warn!(
        path_hash = %path_hash(path),
        path_file_name = ?path.file_name(),
        failure_kind = ?failure_kind,
        "explicit config load failed"
    );
}

/// Emit `message` with bounded fields identifying `path`.
fn debug_config_path(message: &'static str, path: &Path) {
    debug!(
        path_hash = %path_hash(path),
        path_file_name = ?path.file_name(),
        message
    );
}

/// Emit `message` with presence and bounded fields for an optional path string.
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
/// This bounds log cardinality; it is not a cryptographic digest and must not
/// be used as a security boundary.
fn short_hash(value: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Return the bounded correlation hash for `path`.
fn path_hash(path: &Path) -> String {
    short_hash(path.to_string_lossy().as_bytes())
}

/// Return a comparable key for `path`, resolving it where the file exists.
///
/// `OrthoConfig` canonicalises every layer path it records, whereas the expected
/// project path is joined from the caller's `--directory` verbatim. Passing both
/// sides through this function keeps a relative or symlinked directory from
/// looking like a different file.
fn normalized_path_key(path: &str) -> PathBuf {
    let candidate = Path::new(path);
    std::fs::canonicalize(candidate).unwrap_or_else(|_| candidate.to_path_buf())
}

#[cfg(test)]
mod tests {
    //! Unit tests for config discovery through injected environment access.

    use super::*;
    use crate::cli::test_support::TestEnv;
    use anyhow::ensure;
    use cap_std::{ambient_authority, fs::Dir};
    use rstest::rstest;
    use tempfile::tempdir;

    #[test]
    fn env_config_path_returns_none_when_var_unset() {
        let env = TestEnv::default();
        assert!(env_config_path(&env, "__NETSUKE_TEST_VAR").is_none());
    }

    #[test]
    fn env_config_path_returns_none_when_var_empty() {
        let env = TestEnv::default().with_var("__NETSUKE_TEST_VAR", "");
        assert!(env_config_path(&env, "__NETSUKE_TEST_VAR").is_none());
    }

    #[test]
    fn env_config_path_returns_path_when_var_set() {
        let env = TestEnv::default().with_var("__NETSUKE_TEST_VAR", "/tmp/foo.toml");
        let result = env_config_path(&env, "__NETSUKE_TEST_VAR");
        assert_eq!(result, Some(PathBuf::from("/tmp/foo.toml")));
    }

    #[rstest]
    #[case::cli_wins_over_env(
        Some("/env/path.toml"),
        Some("/cli/path.toml"),
        Some("/cli/path.toml")
    )]
    #[case::env_used_without_cli(Some("/env/path.toml"), None, Some("/env/path.toml"))]
    #[case::none_when_sources_missing(None, None, None)]
    fn explicit_config_path_obeys_precedence(
        #[case] env_path: Option<&'static str>,
        #[case] cli_path: Option<&'static str>,
        #[case] expected: Option<&'static str>,
    ) {
        let mut env = TestEnv::default();
        if let Some(path) = env_path {
            env = env.with_var(CONFIG_ENV_VAR, path);
        }
        let cli = Cli {
            config: cli_path.map(PathBuf::from),
            ..Cli::default()
        };

        assert_eq!(
            explicit_config_path_with_env(&cli, &env),
            expected.map(PathBuf::from)
        );
    }

    #[test]
    fn collect_diag_file_layers_uses_injected_explicit_config() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join("netsuke.toml");
        let config_dir = Dir::open_ambient_dir(dir.path(), ambient_authority())?;
        config_dir.write("netsuke.toml", b"json = true\n")?;

        let env = TestEnv::default().with_var(CONFIG_ENV_VAR, config_path.as_os_str());
        let layers = collect_diag_file_layers_with_env(&Cli::default(), &env)?;
        let expected_path = config_path.to_string_lossy().into_owned();

        ensure!(
            layers.iter().any(|layer| layer
                .path()
                .is_some_and(|path| path.as_str() == expected_path)),
            "should include the injected explicit config layer at {expected_path}"
        );

        Ok(())
    }
}

/// Tests for explicit config-path precedence. Enumerated cases cover every
/// combination of `--config` and `NETSUKE_CONFIG` presence; a proptest property
/// test asserts the invariant for generated path values.
#[cfg(test)]
#[path = "config_path_precedence_tests.rs"]
mod config_path_precedence_tests;
