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
const CONFIG_ENV_VAR_LEGACY: &str = "NETSUKE_CONFIG_PATH";

/// Provides access to environment variables used during config discovery.
///
/// Production code uses [`StdEnvProvider`]. Tests can provide a small in-memory
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
    let layers_result = explicit_config_path_with_env(cli, env).map_or_else(
        || collect_file_layers(cli.directory.as_deref()),
        |path| load_layers_from_path(&path),
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

pub(crate) fn explicit_config_path_with_env(
    cli: &Cli,
    env: &impl EnvProvider,
) -> Option<PathBuf> {
    cli.config
        .clone()
        .or_else(|| env_config_path(env, CONFIG_ENV_VAR))
        .or_else(|| env_config_path(env, CONFIG_ENV_VAR_LEGACY))
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

pub(crate) fn collect_diag_file_layers(cli: &Cli) -> OrthoResult<Vec<MergeLayer<'static>>> {
    collect_diag_file_layers_with_env(cli, &StdEnvProvider)
}

pub(crate) fn collect_diag_file_layers_with_env(
    cli: &Cli,
    env: &impl EnvProvider,
) -> OrthoResult<Vec<MergeLayer<'static>>> {
    explicit_config_path_with_env(cli, env).map_or_else(
        || collect_file_layers(cli.directory.as_deref()),
        |path| load_layers_from_path(&path),
    )
}

#[cfg(test)]
mod tests {
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
        Some("/legacy/path.toml"),
        Some("/cli/path.toml"),
        Some("/cli/path.toml")
    )]
    #[case::env_wins_over_legacy(
        Some("/env/path.toml"),
        Some("/legacy/path.toml"),
        None,
        Some("/env/path.toml")
    )]
    #[case::legacy_used_when_primary_missing(
        None,
        Some("/legacy/path.toml"),
        None,
        Some("/legacy/path.toml")
    )]
    #[case::none_when_all_sources_missing(None, None, None, None)]
    fn explicit_config_path_obeys_precedence(
        #[case] primary_env: Option<&'static str>,
        #[case] legacy_env: Option<&'static str>,
        #[case] cli_path: Option<&'static str>,
        #[case] expected: Option<&'static str>,
    ) {
        let mut env = TestEnv::default();
        if let Some(path) = primary_env {
            env = env.with_var(CONFIG_ENV_VAR, path);
        }
        if let Some(path) = legacy_env {
            env = env.with_var(CONFIG_ENV_VAR_LEGACY, path);
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

    #[rstest]
    #[case::primary_env(CONFIG_ENV_VAR)]
    #[case::legacy_env(CONFIG_ENV_VAR_LEGACY)]
    fn collect_diag_file_layers_uses_injected_explicit_config(
        #[case] config_var: &'static str,
    ) -> anyhow::Result<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join("netsuke.toml");
        std::fs::write(&config_path, "diag_json = true\n")?;

        let env = TestEnv::default().with_var(config_var, config_path.as_os_str());
        let layers = collect_diag_file_layers_with_env(&Cli::default(), &env)?;

        ensure!(
            !layers.is_empty(),
            "should include the injected explicit config layer"
        );

        Ok(())
    }
}
