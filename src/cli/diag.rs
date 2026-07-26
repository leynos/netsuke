//! JSON preference resolution from config layers.
//!
//! These helpers determine the effective `json` setting by examining config
//! file layers, environment variables, and CLI matches before the full
//! configuration merge runs, so startup and merge-time failures can still
//! honour the user's diagnostic-output preference.

use clap::ArgMatches;
use clap::parser::ValueSource;
use ortho_config::{OrthoError, OrthoResult};
use serde_json::Value;
use std::sync::Arc;

use super::discovery::{EnvProvider, StdEnvProvider, collect_diag_file_layers_with_env};
use super::parser::Cli;

const JSON_ENV_VAR: &str = "NETSUKE_JSON";

/// Resolve the effective JSON preference from the raw config layers.
///
/// This is used before full config merging so startup and merge-time failures
/// can still honour `json` values sourced from config files or the environment.
///
/// # Errors
///
/// Returns an [`ortho_config::OrthoError`] when a selected config file cannot
/// be loaded, or when `NETSUKE_JSON` contains an invalid boolean.
pub fn resolve_merged_json(cli: &Cli, matches: &ArgMatches) -> OrthoResult<bool> {
    resolve_merged_json_with_env(cli, matches, &StdEnvProvider)
}

/// Resolve the JSON preference using an injected environment provider.
///
/// This variant supports deterministic environment access without mutating
/// process-global state.
///
/// # Errors
///
/// Returns an [`ortho_config::OrthoError`] when a selected config file cannot
/// be loaded, or when `NETSUKE_JSON` contains an invalid boolean.
pub fn resolve_merged_json_with_env(
    cli: &Cli,
    matches: &ArgMatches,
    env: &impl EnvProvider,
) -> OrthoResult<bool> {
    let mut json = json_from_file_layers(cli, env)?;
    if !has_cli_json_override(matches)
        && let Some(env_json) = json_from_env(env)?
    {
        json = env_json;
    }
    Ok(json_from_matches(cli, matches, json))
}

fn json_from_layer(value: &Value) -> Option<bool> {
    value
        .as_object()
        .and_then(|map| map.get("json"))
        .and_then(Value::as_bool)
}

fn json_from_matches(cli: &Cli, matches: &ArgMatches, discovered: bool) -> bool {
    if has_cli_json_override(matches) {
        cli.json
    } else {
        discovered
    }
}

fn has_cli_json_override(matches: &ArgMatches) -> bool {
    matches.value_source("json") == Some(ValueSource::CommandLine)
}

fn json_from_file_layers(cli: &Cli, env: &impl EnvProvider) -> OrthoResult<bool> {
    let default = Cli::default().json;
    let layers = collect_diag_file_layers_with_env(cli, env)?;
    let mut json = default;
    for layer in layers {
        if let Some(layer_json) = json_from_layer(&layer.into_value()) {
            json = layer_json;
        }
    }
    Ok(json)
}

fn json_from_env(env: &impl EnvProvider) -> OrthoResult<Option<bool>> {
    let Some(value) = env.get(JSON_ENV_VAR) else {
        return Ok(None);
    };
    let raw = value.into_string().map_err(|invalid_value| {
        Arc::new(OrthoError::Validation {
            key: String::from(JSON_ENV_VAR),
            message: format!(
                "{JSON_ENV_VAR} must be valid Unicode, got {}",
                invalid_value.to_string_lossy()
            ),
        })
    })?;
    match raw.as_str() {
        "true" | "1" => Ok(Some(true)),
        "false" | "0" => Ok(Some(false)),
        _ => Err(Arc::new(OrthoError::Validation {
            key: String::from(JSON_ENV_VAR),
            message: format!("{JSON_ENV_VAR} must be true, false, 1, or 0; got {raw:?}"),
        })),
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for early JSON preference resolution.

    use super::*;
    use crate::cli::test_support::TestEnv;
    use anyhow::ensure;
    use cap_std::{ambient_authority, fs::Dir};
    use clap::CommandFactory;
    use clap::Parser;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn json_from_layer_reads_json_bool() {
        assert_eq!(json_from_layer(&json!({ "json": true })), Some(true));
        assert_eq!(json_from_layer(&json!({ "json": false })), Some(false));
    }

    #[test]
    fn json_from_layer_ignores_non_bool_json() {
        assert_eq!(json_from_layer(&json!({ "json": "yes" })), None);
    }

    #[test]
    fn resolve_merged_json_reads_injected_env() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join("netsuke.toml");
        let config_dir = Dir::open_ambient_dir(dir.path(), ambient_authority())?;
        config_dir.write("netsuke.toml", b"json = false\n")?;
        let matches = Cli::command().get_matches_from(["netsuke"]);
        let cli = Cli {
            config: Some(config_path),
            ..Cli::default()
        };
        let env = TestEnv::default().with_var(JSON_ENV_VAR, "true");

        ensure!(
            resolve_merged_json_with_env(&cli, &matches, &env)?,
            "injected env should enable JSON"
        );

        Ok(())
    }

    #[test]
    fn resolve_merged_json_rejects_malformed_injected_env() {
        let dir = tempdir().expect("tempdir");
        let config_path = dir.path().join("netsuke.toml");
        let config_dir = Dir::open_ambient_dir(dir.path(), ambient_authority())
            .expect("open temp config directory");
        config_dir.write("netsuke.toml", b"").expect("write config");
        let matches = Cli::command().get_matches_from(["netsuke"]);
        let cli = Cli {
            config: Some(config_path),
            ..Cli::default()
        };
        let env = TestEnv::default().with_var(JSON_ENV_VAR, "yes");

        let error = resolve_merged_json_with_env(&cli, &matches, &env)
            .expect_err("invalid JSON env value should fail");
        assert!(
            matches!(error.as_ref(), OrthoError::Validation { key, .. } if key == JSON_ENV_VAR),
            "expected validation error for {JSON_ENV_VAR}, got {error:?}"
        );
    }

    #[test]
    fn resolve_merged_json_rejects_missing_injected_explicit_config() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let missing_config_path = dir.path().join("missing-netsuke.toml");
        let matches = Cli::command().get_matches_from(["netsuke"]);
        let env = TestEnv::default().with_var("NETSUKE_CONFIG", &missing_config_path);

        let error = resolve_merged_json_with_env(&Cli::default(), &matches, &env)
            .expect_err("missing injected explicit config should fail");
        ensure!(
            matches!(error.as_ref(), OrthoError::File { path, .. } if path == &missing_config_path),
            "expected missing explicit config error for {missing_config_path:?}, got {error:?}"
        );

        Ok(())
    }

    #[test]
    fn resolve_merged_json_honours_cli_before_malformed_env() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join("netsuke.toml");
        let config_dir = Dir::open_ambient_dir(dir.path(), ambient_authority())?;
        config_dir.write("netsuke.toml", b"json = false\n")?;
        let config_path = config_path
            .to_str()
            .expect("temp config path should be UTF-8");
        let args = ["netsuke", "--config", config_path, "--json"];
        let cli = Cli::parse_from(args);
        let matches = Cli::command().get_matches_from(args);
        let env = TestEnv::default().with_var(JSON_ENV_VAR, "yes");

        ensure!(
            resolve_merged_json_with_env(&cli, &matches, &env)?,
            "CLI --json should override malformed JSON env"
        );

        Ok(())
    }
}
