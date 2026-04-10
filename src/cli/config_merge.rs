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
    ConfigDiscovery, LocalizationArgs, MergeComposer, MergeLayer, OrthoMergeExt, OrthoResult,
    load_config_file_as_chain, sanitize_value,
};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::localization::{self, keys};

use super::config::OutputFormat;
use super::{CONFIG_ENV_VAR, Cli, ENV_PREFIX, validation_message};

/// Return the default manifest filename when none is provided.
pub(super) fn default_manifest_path() -> PathBuf {
    PathBuf::from("Netsukefile")
}

/// Return the prefixed environment provider for CLI configuration.
fn env_provider() -> Env {
    Env::prefixed(ENV_PREFIX)
}

/// Build the single-pass discovery used when `NETSUKE_CONFIG_PATH` is set.
///
/// When the env var is present `compose_layers` will find it first and return
/// immediately, so project-vs-user ordering is irrelevant.
fn config_discovery(directory: Option<&Path>) -> ConfigDiscovery {
    let mut builder = ConfigDiscovery::builder("netsuke").env_var(CONFIG_ENV_VAR);
    if let Some(dir) = directory {
        builder = builder.clear_project_roots().add_project_root(dir);
    }
    builder.build()
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
/// # Errors
///
/// Returns an error if project-scope config file loading fails.
fn collect_diag_file_layers(
    directory: Option<&Path>,
) -> Result<Vec<MergeLayer<'static>>, Arc<ortho_config::OrthoError>> {
    let discovery = config_discovery(directory);
    let file_layers = discovery.compose_layers().value;
    let project_file = project_scope_file_str(directory);
    let first_pass_found_project = file_layers.iter().any(|l| {
        l.path()
            .is_some_and(|p| project_file.as_deref() == Some(p.as_str()))
    });
    let has_explicit_config = std::env::var_os(CONFIG_ENV_VAR).is_some_and(|v| !v.is_empty());
    if first_pass_found_project || has_explicit_config {
        Ok(file_layers)
    } else {
        Ok(file_layers
            .into_iter()
            .chain(project_scope_layers(directory)?)
            .collect())
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

    if let Ok(layers) = collect_diag_file_layers(cli.directory.as_deref()) {
        for layer in layers {
            let layer_value = layer.into_value();
            if let Some(layer_diag_json) = diag_json_from_layer(&layer_value) {
                diag_json = layer_diag_json;
            }
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
/// explicit config-path override is active.
fn push_file_layers(
    composer: &mut MergeComposer,
    errors: &mut Vec<Arc<ortho_config::OrthoError>>,
    directory: Option<&Path>,
) {
    let discovery = config_discovery(directory);
    let mut file_layers = discovery.compose_layers();
    errors.append(&mut file_layers.required_errors);
    if file_layers.value.is_empty() {
        errors.append(&mut file_layers.optional_errors);
    }

    let project_file = project_scope_file_str(directory);
    let first_pass_found_project = file_layers.value.iter().any(|l| {
        l.path()
            .is_some_and(|p| project_file.as_deref() == Some(p.as_str()))
    });

    for layer in file_layers.value {
        composer.push_layer(layer);
    }

    let has_explicit_config = std::env::var_os(CONFIG_ENV_VAR).is_some_and(|v| !v.is_empty());
    if !first_pass_found_project && !has_explicit_config {
        match project_scope_layers(directory) {
            Ok(layers) => {
                for layer in layers {
                    composer.push_layer(layer);
                }
            }
            Err(err) => errors.push(err),
        }
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

    push_file_layers(&mut composer, &mut errors, cli.directory.as_deref());

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
