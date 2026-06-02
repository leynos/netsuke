//! Configuration file discovery and loading helpers.
//!
//! This module locates `OrthoConfig` file layers by scanning for config files
//! through [`ConfigDiscovery`], handling explicit paths from CLI flags and
//! environment variables, and loading TOML chains into [`MergeLayer`] values.

use ortho_config::{
    ConfigDiscovery, MergeComposer, MergeLayer, OrthoResult, load_config_file_as_chain,
};
use std::borrow::Cow;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::parser::Cli;

const CONFIG_ENV_VAR: &str = "NETSUKE_CONFIG";
const CONFIG_ENV_VAR_LEGACY: &str = "NETSUKE_CONFIG_PATH";

pub(crate) fn push_file_layers(
    cli: &Cli,
    composer: &mut MergeComposer,
    errors: &mut Vec<Arc<ortho_config::OrthoError>>,
) {
    let layers_result = explicit_config_path(cli).map_or_else(
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
    cli.config
        .clone()
        .or_else(|| env_config_path(CONFIG_ENV_VAR))
        .or_else(|| env_config_path(CONFIG_ENV_VAR_LEGACY))
}

fn env_config_path(var_name: &str) -> Option<PathBuf> {
    std::env::var_os(var_name)
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
    explicit_config_path(cli).map_or_else(
        || collect_file_layers(cli.directory.as_deref()),
        |path| load_layers_from_path(&path),
    )
}
