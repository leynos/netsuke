//! CLI configuration discovery and merge helpers.
//!
//! This module keeps config-layer plumbing out of `cli::mod` so the public CLI
//! surface stays focused on argument definitions and parsing.

use clap::ArgMatches;
use clap::parser::ValueSource;
use ortho_config::declarative::LayerComposition;
use ortho_config::figment::{Figment, providers::Env};
use ortho_config::uncased::Uncased;
use ortho_config::{
    ConfigDiscovery, LocalizationArgs, MergeComposer, MergeLayer, OrthoError, OrthoMergeExt,
    OrthoResult, load_config_file_as_chain, sanitize_value,
};
use std::borrow::Cow;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::localization::{self, keys};

use super::config::OutputFormat;
use super::{CONFIG_ENV_VAR, CONFIG_ENV_VAR_LEGACY, Cli, ENV_PREFIX, validation_message};

/// Return the default manifest filename when none is provided.
pub(super) fn default_manifest_path() -> PathBuf {
    PathBuf::from("Netsukefile")
}

/// Return the prefixed environment provider for CLI configuration.
fn env_provider() -> Env {
    Env::prefixed(ENV_PREFIX)
}

/// Build the automatic discovery used when no explicit config path is set.
fn config_discovery(directory: Option<&Path>) -> ConfigDiscovery {
    let mut builder = ConfigDiscovery::builder("netsuke");
    if let Some(dir) = directory {
        builder = builder.clear_project_roots().add_project_root(dir);
    }
    builder.build()
}

fn env_config_path(var_name: &str) -> Option<PathBuf> {
    std::env::var_os(var_name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn resolve_config_path(cli: &Cli) -> Option<PathBuf> {
    cli.config
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| env_config_path(CONFIG_ENV_VAR))
        .or_else(|| env_config_path(CONFIG_ENV_VAR_LEGACY))
}

fn load_layers_from_path(path: &Path) -> OrthoResult<Vec<MergeLayer<'static>>> {
    match load_config_file_as_chain(path) {
        Ok(Some(chain)) => Ok(chain
            .values
            .into_iter()
            .map(|(value, layer_path)| MergeLayer::file(Cow::Owned(value), Some(layer_path)))
            .collect()),
        Ok(None) => Err(Arc::new(OrthoError::File {
            path: path.to_path_buf(),
            source: Box::new(io::Error::new(
                io::ErrorKind::NotFound,
                "explicit configuration file not found",
            )),
        })),
        Err(err) => Err(err),
    }
}

/// Return the expected project-scope config file path as a string, if
/// resolvable.
fn project_scope_file_str(directory: Option<&Path>) -> Option<String> {
    let root = directory
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())?;
    root.join(".netsuke.toml").to_str().map(String::from)
}

/// Load the project-scope config file directly, bypassing discovery.
///
/// Returns layers from the project `.netsuke.toml` (including any `extends`
/// chain) if the file exists, or an empty vec if the file does not exist.
///
/// # Errors
///
/// Returns an error if the config file exists but cannot be parsed or if an
/// `extends` chain is malformed.
fn project_scope_layers(
    directory: Option<&Path>,
) -> Result<Vec<MergeLayer<'static>>, Arc<ortho_config::OrthoError>> {
    let root = directory
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok());
    let Some(project_file) = root.map(|d| d.join(".netsuke.toml")) else {
        return Ok(Vec::new());
    };
    match load_config_file_as_chain(&project_file) {
        Ok(Some(chain)) => Ok(chain
            .values
            .into_iter()
            .map(|(value, path)| MergeLayer::file(Cow::Owned(value), Some(path)))
            .collect()),
        Ok(None) => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

/// Return `true` when no CLI overrides were supplied.
///
/// The merge pipeline treats an empty JSON object as "no overrides".
fn is_empty_value(value: &serde_json::Value) -> bool {
    matches!(value, serde_json::Value::Object(map) if map.is_empty())
}

fn diag_json_from_layer(value: &serde_json::Value) -> Option<bool> {
    let map = value.as_object()?;
    if let Some(output_format) = map
        .get("output_format")
        .and_then(serde_json::Value::as_str)
        .and_then(|format| OutputFormat::parse_raw(format).ok())
    {
        return Some(output_format.is_json());
    }
    map.get("diag_json").and_then(serde_json::Value::as_bool)
}

/// Collect config-file layers in precedence order for diagnostic-JSON resolution.
///
/// Mirrors the two-pass logic of [`push_file_layers`] without a `MergeComposer`.
///
/// If project-scope layer loading fails, this function falls back to the
/// first-pass layers (global and user configs) rather than propagating an
/// error. An explicit config path returns the selected file's layers, or an
/// empty set when the selected file cannot be loaded.
fn collect_diag_file_layers(cli: &Cli) -> Vec<MergeLayer<'static>> {
    if let Some(path) = resolve_config_path(cli) {
        return load_layers_from_path(&path).unwrap_or_default();
    }

    let discovery = config_discovery(cli.directory.as_deref());
    let file_layers = discovery.compose_layers().value;
    let project_file = project_scope_file_str(cli.directory.as_deref());
    let first_pass_found_project = file_layers.iter().any(|l| {
        l.path()
            .is_some_and(|p| project_file.as_deref() == Some(p.as_str()))
    });
    if first_pass_found_project {
        file_layers
    } else {
        match project_scope_layers(cli.directory.as_deref()) {
            Ok(project_layers) => file_layers.into_iter().chain(project_layers).collect(),
            Err(_) => file_layers,
        }
    }
}

/// Resolve the final `diag_json` preference from CLI flag matches,
/// falling back to `discovered` when no CLI flag was explicitly set.
fn diag_json_from_matches(cli: &Cli, matches: &ArgMatches, discovered: bool) -> bool {
    if matches.value_source("output_format") == Some(ValueSource::CommandLine) {
        cli.resolved_diag_json()
    } else if matches.value_source("diag_json") == Some(ValueSource::CommandLine) {
        cli.diag_json
    } else {
        discovered
    }
}

/// Resolve the effective diagnostic JSON preference from the raw config layers.
///
/// This is used before full config merging so startup and merge-time failures
/// can still honour `diag_json` values sourced from config files or the
/// environment.
#[must_use]
pub fn resolve_merged_diag_json(cli: &Cli, matches: &ArgMatches) -> bool {
    let mut diag_json = Cli::default().diag_json;

    let layers = collect_diag_file_layers(cli);
    for layer in layers {
        let layer_value = layer.into_value();
        if let Some(layer_diag_json) = diag_json_from_layer(&layer_value) {
            diag_json = layer_diag_json;
        }
    }

    let env_provider = env_provider()
        .map(|key| Uncased::new(key.as_str().to_ascii_uppercase()))
        .split("__");
    if let Ok(value) = Figment::from(env_provider).extract::<serde_json::Value>()
        && let Some(env_diag_json) = diag_json_from_layer(&value)
    {
        diag_json = env_diag_json;
    }

    diag_json_from_matches(cli, matches, diag_json)
}

/// Push all config-file layers onto `composer` in the correct precedence order.
///
/// Implements "project scope > user scope" by running a second direct load of
/// the project-scope file when first-pass discovery did not include it and no
/// explicit config path is active.
///
/// Drain a layer-load result onto `composer`, recording any error.
fn push_layers_result(
    composer: &mut MergeComposer,
    errors: &mut Vec<Arc<ortho_config::OrthoError>>,
    result: Result<Vec<MergeLayer<'static>>, Arc<ortho_config::OrthoError>>,
) {
    match result {
        Ok(layers) => {
            for layer in layers {
                composer.push_layer(layer);
            }
        }
        Err(err) => errors.push(err),
    }
}

fn push_file_layers(
    composer: &mut MergeComposer,
    errors: &mut Vec<Arc<ortho_config::OrthoError>>,
    cli: &Cli,
) {
    if let Some(path) = resolve_config_path(cli) {
        push_layers_result(composer, errors, load_layers_from_path(&path));
        return;
    }

    let discovery = config_discovery(cli.directory.as_deref());
    let mut file_layers = discovery.compose_layers();
    errors.append(&mut file_layers.required_errors);
    if file_layers.value.is_empty() {
        errors.append(&mut file_layers.optional_errors);
    }

    let project_file = project_scope_file_str(cli.directory.as_deref());
    let first_pass_found_project = file_layers.value.iter().any(|l| {
        l.path()
            .is_some_and(|p| project_file.as_deref() == Some(p.as_str()))
    });

    for layer in file_layers.value {
        composer.push_layer(layer);
    }

    if !first_pass_found_project {
        push_layers_result(
            composer,
            errors,
            project_scope_layers(cli.directory.as_deref()),
        );
    }
}

fn cli_overrides_from_matches(cli: &Cli, matches: &ArgMatches) -> OrthoResult<serde_json::Value> {
    let value = sanitize_value(cli)?;
    let mut map = match value {
        serde_json::Value::Object(map) => map,
        other => {
            let mut args = LocalizationArgs::default();
            args.insert("value", format!("{other:?}").into());
            let localizer = localization::localizer();
            return Err(Arc::new(ortho_config::OrthoError::Validation {
                key: String::from("cli"),
                message: validation_message(
                    localizer.as_ref(),
                    keys::CLI_CONFIG_EXPECTED_OBJECT,
                    Some(&args),
                    &format!("expected parsed CLI values to serialize to an object, got {other:?}"),
                ),
            }));
        }
    };

    map.remove("command");
    for field in [
        "file",
        "verbose",
        "locale",
        "fetch_default_deny",
        "fetch_allow_scheme",
        "fetch_allow_host",
        "fetch_block_host",
        "accessible",
        "progress",
        "no_emoji",
        "theme",
        "colour_policy",
        "spinner_mode",
        "diag_json",
        "output_format",
        "default_targets",
    ] {
        if matches.value_source(field) != Some(ValueSource::CommandLine) {
            map.remove(field);
        }
    }

    Ok(serde_json::Value::Object(map))
}

/// Merge configuration layers over the parsed CLI values.
///
/// # Errors
///
/// Returns an [`ortho_config::OrthoError`] if layer composition or merging
/// fails.
pub fn merge_with_config(cli: &Cli, matches: &ArgMatches) -> OrthoResult<Cli> {
    let command = cli.command.clone();
    let mut errors = Vec::new();
    let mut composer = MergeComposer::with_capacity(4);

    match sanitize_value(&Cli::default()) {
        Ok(value) => composer.push_defaults(value),
        Err(err) => errors.push(err),
    }

    push_file_layers(&mut composer, &mut errors, cli);

    let env_provider = env_provider()
        .map(|key| Uncased::new(key.as_str().to_ascii_uppercase()))
        .split("__");
    match Figment::from(env_provider)
        .extract::<serde_json::Value>()
        .into_ortho_merge()
    {
        Ok(value) => composer.push_environment(value),
        Err(err) => errors.push(err),
    }

    match cli_overrides_from_matches(cli, matches) {
        Ok(value) if !is_empty_value(&value) => composer.push_cli(value),
        Ok(_) => {}
        Err(err) => errors.push(err),
    }

    let composition = LayerComposition::new(composer.layers(), errors);
    let mut merged = composition.into_merge_result(Cli::merge_from_layers)?;
    merged.command = command;
    Ok(merged)
}

#[cfg(test)]
#[path = "config_merge_tests.rs"]
mod tests;
