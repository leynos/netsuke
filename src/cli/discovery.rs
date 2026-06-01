//! Configuration file discovery and loading helpers.
//!
//! This module locates `OrthoConfig` file layers by scanning for config files
//! through [`ConfigDiscovery`], handling explicit paths from CLI flags and
//! environment variables, and loading TOML chains into [`MergeLayer`] values.

use ortho_config::{
    ConfigDiscovery, MergeComposer, MergeLayer, OrthoResult, load_config_file_as_chain,
};
use std::borrow::Cow;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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
    explicit_config_path_with_env(cli, env).map_or_else(
        || collect_file_layers(cli.directory.as_deref()),
        |path| load_layers_from_path(&path),
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
    let has_project_layer = file_layers.value.iter().any(|layer| {
        layer
            .path()
            .is_some_and(|path| project_file.as_deref() == Some(path.as_str()))
    });
    if has_project_layer {
        return Ok(file_layers.value);
    }

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

pub(crate) fn explicit_config_path(cli: &Cli) -> Option<PathBuf> {
    explicit_config_path_with_env(cli, &StdEnvProvider)
}

pub(crate) fn explicit_config_path_with_env(cli: &Cli, env: &impl EnvProvider) -> Option<PathBuf> {
    cli.config
        .clone()
        .or_else(|| env_config_path(env, CONFIG_ENV_VAR))
}

fn env_config_path(env: &impl EnvProvider, var_name: &str) -> Option<PathBuf> {
    env.get(var_name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub(crate) fn load_layers_from_path(
    path: &std::path::Path,
) -> OrthoResult<Vec<MergeLayer<'static>>> {
    match load_config_file_as_chain(path) {
        Ok(Some(chain)) => Ok(chain
            .values
            .into_iter()
            .map(|(value, layer_path)| MergeLayer::file(Cow::Owned(value), Some(layer_path)))
            .collect()),
        Ok(None) => Err(Arc::new(ortho_config::OrthoError::File {
            path: path.to_path_buf(),
            source: Box::new(io::Error::new(
                io::ErrorKind::NotFound,
                "explicit configuration file not found",
            )),
        })),
        Err(err) => Err(err),
    }
}

pub(crate) fn collect_diag_file_layers_with_env(
    cli: &Cli,
    env: &impl EnvProvider,
) -> OrthoResult<Vec<MergeLayer<'static>>> {
    collect_file_layers_with_env(cli, env)
}

#[cfg(test)]
mod tests {
    //! Unit tests for config discovery through injected environment access.

    use super::*;
    use anyhow::ensure;
    use rstest::rstest;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[derive(Default)]
    struct TestEnv {
        values: HashMap<&'static str, OsString>,
    }

    impl TestEnv {
        fn with_var(mut self, name: &'static str, value: impl Into<OsString>) -> Self {
            self.values.insert(name, value.into());
            self
        }
    }

    impl EnvProvider for TestEnv {
        fn get(&self, key: &str) -> Option<OsString> {
            self.values.get(key).cloned()
        }
    }

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
        std::fs::write(&config_path, "json = true\n")?;

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
