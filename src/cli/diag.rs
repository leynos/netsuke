//! Diagnostic JSON preference resolution from config layers.
//!
//! These helpers determine the effective `diag_json` setting by examining
//! config file layers, environment variables, and CLI matches before the
//! full configuration merge runs, so startup and merge-time failures can
//! still honour the user's diagnostic-output preference.

use clap::ArgMatches;
use clap::parser::ValueSource;
use ortho_config::OrthoResult;
use ortho_config::figment::Figment;
use ortho_config::uncased::Uncased;
use serde_json::Value;

use super::discovery::collect_diag_file_layers;
use super::merge::env_provider;
use super::parser::Cli;

/// Resolve the effective diagnostic JSON preference from the raw config layers.
///
/// This is used before full config merging so startup and merge-time failures
/// can still honour `diag_json` values sourced from config files or the
/// environment.
#[must_use]
pub fn resolve_merged_diag_json(cli: &Cli, matches: &ArgMatches) -> bool {
    let mut diag_json =
        diag_json_from_file_layers(cli).unwrap_or_else(|_| Cli::default().diag_json);
    diag_json = diag_json_from_env(diag_json);
    diag_json_from_matches(cli, matches, diag_json)
}

fn diag_json_from_layer(value: &Value) -> Option<bool> {
    value
        .as_object()
        .and_then(|map| map.get("diag_json"))
        .and_then(Value::as_bool)
}

fn diag_json_from_matches(cli: &Cli, matches: &ArgMatches, discovered: bool) -> bool {
    if matches.value_source("output_format") == Some(ValueSource::CommandLine) {
        cli.resolved_diag_json()
    } else if matches.value_source("diag_json") == Some(ValueSource::CommandLine) {
        cli.diag_json
    } else {
        discovered
    }
}

fn diag_json_from_file_layers(cli: &Cli) -> OrthoResult<bool> {
    let default = Cli::default().diag_json;
    let layers = collect_diag_file_layers(cli)?;
    let mut diag_json = default;
    for layer in layers {
        if let Some(layer_diag_json) = diag_json_from_layer(&layer.into_value()) {
            diag_json = layer_diag_json;
        }
    }
    Ok(diag_json)
}

fn diag_json_from_env(fallback: bool) -> bool {
    let env_provider = env_provider()
        .map(|key| Uncased::new(key.as_str().to_ascii_uppercase()))
        .split("__");
    Figment::from(env_provider)
        .extract::<serde_json::Value>()
        .ok()
        .and_then(|value| diag_json_from_layer(&value))
        .unwrap_or(fallback)
}
