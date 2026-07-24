//! Diagnostic JSON preference resolution from config layers.
//!
//! These helpers determine the effective `diag_json` setting by examining
//! config file layers, environment variables, and CLI matches before the
//! full configuration merge runs, so startup and merge-time failures can
//! still honour the user's diagnostic-output preference.

use clap::ArgMatches;
use clap::parser::ValueSource;
use ortho_config::{OrthoError, OrthoResult};
use serde_json::Value;
use std::sync::Arc;

use super::discovery::{EnvProvider, StdEnvProvider, collect_diag_file_layers_with_env};
use super::parser::Cli;

/// Resolve the effective diagnostic JSON preference from the raw config layers.
///
/// This is used before full config merging so startup and merge-time failures
/// can still honour `diag_json` values sourced from config files or the
/// environment.
///
/// # Errors
///
/// Returns an [`ortho_config::OrthoError`] when a selected config file cannot
/// be loaded, or when `NETSUKE_DIAG_JSON` contains an invalid boolean.
pub fn resolve_merged_diag_json(cli: &Cli, matches: &ArgMatches) -> OrthoResult<bool> {
    resolve_merged_diag_json_with_env(cli, matches, &StdEnvProvider)
}

/// Resolve diagnostic JSON preference using an injected environment provider.
///
/// This variant is intended for tests and for callers that need deterministic
/// environment access without mutating the process environment.
///
/// # Errors
///
/// Returns an [`ortho_config::OrthoError`] when a selected config file cannot
/// be loaded, or when `NETSUKE_DIAG_JSON` contains an invalid boolean.
pub fn resolve_merged_diag_json_with_env(
    cli: &Cli,
    matches: &ArgMatches,
    env: &impl EnvProvider,
) -> OrthoResult<bool> {
    let mut diag_json = diag_json_from_file_layers(cli, env)?;
    if !has_cli_diag_json_override(matches)
        && let Some(env_diag_json) = diag_json_from_env(env)?
    {
        diag_json = env_diag_json;
    }
    Ok(diag_json_from_matches(cli, matches, diag_json))
}

fn diag_json_from_layer(value: &Value) -> Option<bool> {
    value
        .as_object()
        .and_then(|map| map.get("diag_json"))
        .and_then(Value::as_bool)
}

fn diag_json_from_matches(cli: &Cli, matches: &ArgMatches, discovered: bool) -> bool {
    if has_cli_output_format_override(matches) {
        cli.resolved_diag_json()
    } else if has_cli_diag_json_flag(matches) {
        cli.diag_json
    } else {
        discovered
    }
}

fn has_cli_diag_json_override(matches: &ArgMatches) -> bool {
    has_cli_output_format_override(matches) || has_cli_diag_json_flag(matches)
}

fn has_cli_output_format_override(matches: &ArgMatches) -> bool {
    matches.value_source("output_format") == Some(ValueSource::CommandLine)
}

fn has_cli_diag_json_flag(matches: &ArgMatches) -> bool {
    matches.value_source("diag_json") == Some(ValueSource::CommandLine)
}

fn diag_json_from_file_layers(cli: &Cli, env: &impl EnvProvider) -> OrthoResult<bool> {
    let default = Cli::default().diag_json;
    let layers = collect_diag_file_layers_with_env(cli, env)?;
    let mut diag_json = default;
    for layer in layers {
        if let Some(layer_diag_json) = diag_json_from_layer(&layer.into_value()) {
            diag_json = layer_diag_json;
        }
    }
    Ok(diag_json)
}

fn diag_json_from_env(env: &impl EnvProvider) -> OrthoResult<Option<bool>> {
    let Some(value) = env.get("NETSUKE_DIAG_JSON") else {
        return Ok(None);
    };
    let raw = value.into_string().map_err(|invalid_value| {
        Arc::new(OrthoError::Validation {
            key: String::from("NETSUKE_DIAG_JSON"),
            message: format!(
                "NETSUKE_DIAG_JSON must be valid Unicode, got {}",
                invalid_value.to_string_lossy()
            ),
        })
    })?;
    match raw.as_str() {
        "true" | "1" => Ok(Some(true)),
        "false" | "0" => Ok(Some(false)),
        _ => Err(Arc::new(OrthoError::Validation {
            key: String::from("NETSUKE_DIAG_JSON"),
            message: format!("NETSUKE_DIAG_JSON must be true, false, 1, or 0; got {raw:?}"),
        })),
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for early diagnostic JSON preference resolution.

    use super::*;
    use anyhow::ensure;
    use cap_std::{ambient_authority, fs::Dir};
    use clap::CommandFactory;
    use clap::Parser;
    use serde_json::json;
    use std::collections::HashMap;
    use std::ffi::OsString;
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
    fn diag_json_from_layer_reads_diag_json_bool() {
        assert_eq!(
            diag_json_from_layer(&json!({ "diag_json": true })),
            Some(true)
        );
        assert_eq!(
            diag_json_from_layer(&json!({ "diag_json": false })),
            Some(false)
        );
    }

    #[test]
    fn diag_json_from_layer_ignores_non_bool_diag_json() {
        assert_eq!(diag_json_from_layer(&json!({ "diag_json": "yes" })), None);
    }

    #[test]
    fn resolve_merged_diag_json_reads_injected_env() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join("netsuke.toml");
        let config_dir = Dir::open_ambient_dir(dir.path(), ambient_authority())?;
        config_dir.write("netsuke.toml", b"diag_json = false\n")?;
        let matches = Cli::command().get_matches_from(["netsuke"]);
        let cli = Cli {
            config: Some(config_path),
            ..Cli::default()
        };
        let env = TestEnv::default().with_var("NETSUKE_DIAG_JSON", "true");

        ensure!(
            resolve_merged_diag_json_with_env(&cli, &matches, &env)?,
            "injected env should enable diagnostic JSON"
        );

        Ok(())
    }

    #[test]
    fn resolve_merged_diag_json_rejects_malformed_injected_env() {
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
        let env = TestEnv::default().with_var("NETSUKE_DIAG_JSON", "yes");

        let error = resolve_merged_diag_json_with_env(&cli, &matches, &env)
            .expect_err("invalid diagnostic JSON env value should fail");
        assert!(
            matches!(error.as_ref(), OrthoError::Validation { key, .. } if key == "NETSUKE_DIAG_JSON"),
            "expected validation error for NETSUKE_DIAG_JSON, got {error:?}"
        );
    }

    #[test]
    fn resolve_merged_diag_json_honours_cli_overrides_before_malformed_env() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join("netsuke.toml");
        let config_dir = Dir::open_ambient_dir(dir.path(), ambient_authority())?;
        config_dir.write("netsuke.toml", b"diag_json = false\n")?;
        let config_path_string = config_path
            .to_str()
            .expect("temp config path should be UTF-8");
        let env = TestEnv::default().with_var("NETSUKE_DIAG_JSON", "yes");

        for (description, override_args) in [
            ("--diag-json", &["--diag-json"][..]),
            ("--output-format json", &["--output-format", "json"][..]),
        ] {
            let mut args = vec!["netsuke", "--config", config_path_string];
            args.extend_from_slice(override_args);

            let cli = Cli::parse_from(&args);
            let matches = Cli::command().get_matches_from(&args);

            ensure!(
                resolve_merged_diag_json_with_env(&cli, &matches, &env)?,
                "CLI {description} should override malformed diagnostic JSON env"
            );
        }

        Ok(())
    }
}
