//! JSON preference resolution from config layers.
//!
//! These helpers determine the effective `json` setting by examining config
//! file layers, environment variables, and CLI matches before the full
//! configuration merge runs, so startup and merge-time failures can still
//! honour the user's diagnostic-output preference.

use clap::ArgMatches;
use clap::parser::ValueSource;
use ortho_config::{OrthoError, OrthoResult};
use std::sync::Arc;

use super::discovery::{
    DiscoveredLayers, DiscoveryOutcome, EnvProvider, StdEnvProvider,
    collect_diag_file_layers_with_env,
};
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
    let (json, _) = resolve_json_and_layers_with_env(cli, matches, env)?;
    Ok(json)
}

/// Resolve diagnostic JSON mode and retain the discovered file layers.
///
/// The returned layers belong to this exact resolution pass and must be passed
/// to [`super::merge::merge_with_cached_file_layers`] for the subsequent full
/// merge. This standalone wrapper replays deferred discovery diagnostics before
/// returning. Startup callers that must configure tracing first use
/// [`resolve_json_and_layers_outcome_with_env`] instead.
///
/// # Errors
///
/// Returns the first discovery error immediately, or a validation error when
/// `NETSUKE_JSON` contains an invalid boolean.
pub fn resolve_json_and_layers_with_env(
    cli: &Cli,
    matches: &ArgMatches,
    env: &impl EnvProvider,
) -> OrthoResult<(bool, DiscoveredLayers)> {
    let (result, outcome) = resolve_json_and_layers_outcome_with_env(cli, matches, env);
    outcome.emit_diagnostics();
    result.map(|json| (json, outcome.into_layers()))
}

/// Resolve diagnostic JSON mode while retaining a discovery outcome.
///
/// Startup uses this form to replay cached diagnostics after it enables its
/// output filter, including when discovery or JSON validation fails. The
/// outcome owns the discovered layers and deferred diagnostics. Standalone
/// callers should usually prefer [`resolve_json_and_layers_with_env`].
pub fn resolve_json_and_layers_outcome_with_env(
    cli: &Cli,
    matches: &ArgMatches,
    env: &impl EnvProvider,
) -> (OrthoResult<bool>, DiscoveryOutcome) {
    let outcome = collect_diag_file_layers_with_env(cli, env);
    let result = (|| {
        if let Some(error) = outcome.first_error() {
            return Err(Arc::clone(error));
        }
        let mut json = json_from_layers(&outcome);
        if !has_cli_json_override(matches)
            && let Some(env_json) = json_from_env(env)?
        {
            json = env_json;
        }
        Ok(json_from_matches(cli, matches, json))
    })();
    (result, outcome)
}

#[cfg(test)]
fn json_from_layer(value: &serde_json::Value) -> Option<bool> {
    value
        .as_object()
        .and_then(|map| map.get("json"))
        .and_then(serde_json::Value::as_bool)
}

/// Apply the command-line JSON override to a discovered preference.
fn json_from_matches(cli: &Cli, matches: &ArgMatches, discovered: bool) -> bool {
    if has_cli_json_override(matches) {
        cli.json
    } else {
        discovered
    }
}

/// Determine whether `--json` was supplied on the command line.
fn has_cli_json_override(matches: &ArgMatches) -> bool {
    matches.value_source("json") == Some(ValueSource::CommandLine)
}

/// Resolve the last valid JSON preference from discovered config layers.
const fn json_from_layers(outcome: &DiscoveryOutcome) -> bool {
    outcome.json_preference()
}
/// Parse the optional `NETSUKE_JSON` value supplied by `env`.
///
/// Invalid or non-Unicode values are validation errors rather than silently
/// falling back, so users receive actionable configuration feedback.
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
    use crate::cli::discovery::assert_bounded_path_event;
    use crate::cli::test_support::TestEnv;
    use crate::test_tracing_capture::with_test_subscriber;
    use anyhow::{Context, ensure};
    use cap_std::{ambient_authority, fs::Dir};
    use clap::CommandFactory;
    use clap::Parser;
    use rstest::rstest;
    use serde_json::json;
    use tempfile::tempdir;
    use tracing_subscriber::filter::LevelFilter;

    fn find_deferred_event<'a>(events: &'a [String], message: &str) -> anyhow::Result<&'a str> {
        events
            .iter()
            .find(|event| event.contains(message))
            .map(String::as_str)
            .with_context(|| format!("expected event containing {message:?} in {events:?}"))
    }

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

    #[rstest]
    fn resolve_merged_json_replays_missing_explicit_config_diagnostics() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let missing_config_path = dir.path().join("missing-netsuke.toml");
        let matches = Cli::command().get_matches_from(["netsuke"]);
        let env = TestEnv::default().with_var("NETSUKE_CONFIG", &missing_config_path);

        let (result, events) = with_test_subscriber(LevelFilter::TRACE, |captured| {
            let result = resolve_merged_json_with_env(&Cli::default(), &matches, &env);
            (result, captured.snapshot())
        });
        let error = result.expect_err("missing injected explicit config should fail");
        ensure!(
            matches!(error.as_ref(), OrthoError::File { path, .. } if path == &missing_config_path),
            "expected missing explicit config error for {missing_config_path:?}, got {error:?}"
        );
        let selector_event = find_deferred_event(&events, "resolved config path")?;
        ensure!(
            selector_event.contains("selector=\"NETSUKE_CONFIG\"")
                && selector_event.contains("path_present=true"),
            "selector event should record the injected selector: {selector_event}"
        );
        assert_bounded_path_event(selector_event, &missing_config_path)?;
        assert_bounded_path_event(
            find_deferred_event(&events, "using explicit config path")?,
            &missing_config_path,
        )?;
        let failure_event = find_deferred_event(&events, "explicit config load failed")?;
        ensure!(
            failure_event.contains("failure_kind=Missing"),
            "load failure should retain its bounded kind: {failure_event}"
        );
        assert_bounded_path_event(failure_event, &missing_config_path)?;

        Ok(())
    }

    #[rstest]
    fn resolve_json_and_layers_replays_successful_explicit_config_diagnostics() -> anyhow::Result<()>
    {
        let dir = tempdir()?;
        let config_path = dir.path().join("customer@example.com.toml");
        let config_dir = Dir::open_ambient_dir(dir.path(), ambient_authority())?;
        config_dir.write("customer@example.com.toml", b"json = true\n")?;
        let matches = Cli::command().get_matches_from(["netsuke"]);
        let cli = Cli {
            config: Some(config_path.clone()),
            ..Cli::default()
        };

        let (result, events) = with_test_subscriber(LevelFilter::TRACE, |captured| {
            let result = resolve_json_and_layers_with_env(&cli, &matches, &TestEnv::default());
            (result, captured.snapshot())
        });
        let (json, layers) = result?;
        ensure!(json, "explicit configuration should enable JSON mode");
        drop(layers);
        let selector_event = find_deferred_event(&events, "resolved config path")?;
        ensure!(
            selector_event.contains("selector=\"cli_flag\"")
                && selector_event.contains("path_present=true"),
            "selector event should record the CLI selector: {selector_event}"
        );
        assert_bounded_path_event(selector_event, &config_path)?;
        assert_bounded_path_event(
            find_deferred_event(&events, "using explicit config path")?,
            &config_path,
        )?;

        Ok(())
    }

    #[test]
    fn resolve_merged_json_honours_cli_before_malformed_env() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join("netsuke.toml");
        let config_dir = Dir::open_ambient_dir(dir.path(), ambient_authority())?;
        config_dir.write("netsuke.toml", b"json = false\n")?;
        let config_path_string = config_path
            .to_str()
            .expect("temp config path should be UTF-8");
        let args = ["netsuke", "--config", config_path_string, "--json"];
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
