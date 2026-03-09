//! Layer-composition and conversion helpers for CLI configuration.

use clap::ArgMatches;
use clap::parser::ValueSource;
use ortho_config::declarative::LayerComposition;
use ortho_config::figment::{Figment, providers::Env};
use ortho_config::uncased::Uncased;
use ortho_config::{
    ConfigDiscovery, MergeComposer, OrthoError, OrthoMergeExt, OrthoResult, sanitize_value,
};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::path::PathBuf;
use std::sync::Arc;

use super::config::{BuildConfig, CliConfig, Theme};
use super::parser::{BuildArgs, Cli, Commands};
const CONFIG_ENV_VAR: &str = "NETSUKE_CONFIG_PATH";
const ENV_PREFIX: &str = "NETSUKE_";

/// Merge discovered configuration layers over parsed CLI input.
///
/// # Errors
///
/// Returns an [`ortho_config::OrthoError`] if layer composition or merging
/// fails.
pub fn merge_with_config(cli: &Cli, matches: &ArgMatches) -> OrthoResult<Cli> {
    let mut errors = Vec::new();
    let mut composer = MergeComposer::with_capacity(4);

    match sanitize_value(&CliConfig::default()) {
        Ok(value) => composer.push_defaults(value),
        Err(err) => errors.push(err),
    }

    let discovery = config_discovery(cli.directory.as_ref());
    let mut file_layers = discovery.compose_layers();
    errors.append(&mut file_layers.required_errors);
    if file_layers.value.is_empty() {
        errors.append(&mut file_layers.optional_errors);
    }
    for layer in file_layers.value {
        composer.push_layer(layer);
    }

    let env_provider = env_provider()
        .map(|key| Uncased::new(key.as_str().to_ascii_uppercase()))
        .split("__");
    match Figment::from(env_provider)
        .extract::<Value>()
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
    let merged = composition.into_merge_result(CliConfig::merge_from_layers)?;
    Ok(apply_config(cli, merged))
}

fn env_provider() -> Env {
    Env::prefixed(ENV_PREFIX)
}

fn config_discovery(directory: Option<&PathBuf>) -> ConfigDiscovery {
    let mut builder = ConfigDiscovery::builder("netsuke").env_var(CONFIG_ENV_VAR);
    if let Some(dir) = directory {
        builder = builder.clear_project_roots().add_project_root(dir);
    }
    builder.build()
}

fn is_empty_value(value: &Value) -> bool {
    matches!(value, Value::Object(map) if map.is_empty())
}

fn diag_json_from_layer(value: &Value) -> Option<bool> {
    value
        .as_object()
        .and_then(|map| map.get("diag_json"))
        .and_then(Value::as_bool)
}

/// Resolve the effective diagnostic JSON preference from the raw config layers.
///
/// This is used before full config merging so startup and merge-time failures
/// can still honour `diag_json` values sourced from config files or the
/// environment.
#[must_use]
pub fn resolve_merged_diag_json(cli: &Cli, matches: &ArgMatches) -> bool {
    let mut diag_json = CliConfig::default().diag_json;

    let discovery = config_discovery(cli.directory.as_ref());
    let file_layers = discovery.compose_layers();
    for layer in file_layers.value {
        let layer_value = layer.into_value();
        if let Some(layer_diag_json) = diag_json_from_layer(&layer_value) {
            diag_json = layer_diag_json;
        }
    }

    let env_provider = env_provider()
        .map(|key| Uncased::new(key.as_str().to_ascii_uppercase()))
        .split("__");
    if let Ok(value) = Figment::from(env_provider).extract::<Value>()
        && let Some(env_diag_json) = diag_json_from_layer(&value)
    {
        diag_json = env_diag_json;
    }

    if matches.value_source("diag_json") == Some(ValueSource::CommandLine) {
        cli.diag_json
    } else {
        diag_json
    }
}

fn cli_overrides_from_matches(cli: &Cli, matches: &ArgMatches) -> OrthoResult<Value> {
    let mut root = Map::new();

    maybe_insert_explicit(matches, "file", &cli.file, &mut root)?;
    maybe_insert_explicit(matches, "jobs", &cli.jobs, &mut root)?;
    maybe_insert_explicit(matches, "verbose", &cli.verbose, &mut root)?;
    maybe_insert_explicit(matches, "locale", &cli.locale, &mut root)?;
    maybe_insert_explicit(
        matches,
        "fetch_allow_scheme",
        &cli.fetch_allow_scheme,
        &mut root,
    )?;
    maybe_insert_explicit(
        matches,
        "fetch_allow_host",
        &cli.fetch_allow_host,
        &mut root,
    )?;
    maybe_insert_explicit(
        matches,
        "fetch_block_host",
        &cli.fetch_block_host,
        &mut root,
    )?;
    maybe_insert_explicit(
        matches,
        "fetch_default_deny",
        &cli.fetch_default_deny,
        &mut root,
    )?;
    maybe_insert_explicit(matches, "accessible", &cli.accessible, &mut root)?;
    maybe_insert_explicit(matches, "progress", &cli.progress, &mut root)?;
    maybe_insert_explicit(matches, "no_emoji", &cli.no_emoji, &mut root)?;
    maybe_insert_explicit(matches, "diag_json", &cli.diag_json, &mut root)?;

    if let Some(Commands::Build(args)) = cli.command.as_ref()
        && let Some(build_matches) = matches.subcommand_matches("build")
    {
        let build = build_cli_overrides(args, build_matches)?;
        if !build.is_empty() {
            root.insert("cmds".to_owned(), json!({ "build": Value::Object(build) }));
        }
    }

    Ok(Value::Object(root))
}

fn build_cli_overrides(args: &BuildArgs, matches: &ArgMatches) -> OrthoResult<Map<String, Value>> {
    let mut build = Map::new();
    maybe_insert_explicit(matches, "emit", &args.emit, &mut build)?;
    maybe_insert_explicit(matches, "targets", &args.targets, &mut build)?;
    Ok(build)
}

fn maybe_insert_explicit<T>(
    matches: &ArgMatches,
    field: &str,
    value: &T,
    target: &mut Map<String, Value>,
) -> OrthoResult<()>
where
    T: Serialize,
{
    if matches.value_source(field) == Some(ValueSource::CommandLine) {
        target.insert(field.to_owned(), serialize_value(field, value)?);
    }
    Ok(())
}

fn serialize_value<T>(field: &str, value: &T) -> OrthoResult<Value>
where
    T: Serialize,
{
    serde_json::to_value(value).map_err(|err| validation_error(field, &err.to_string()))
}

fn apply_config(parsed: &Cli, config: CliConfig) -> Cli {
    Cli {
        file: config.file,
        directory: parsed.directory.clone(),
        jobs: config.jobs,
        verbose: config.verbose,
        locale: config.locale,
        fetch_allow_scheme: config.fetch_allow_scheme,
        fetch_allow_host: config.fetch_allow_host,
        fetch_block_host: config.fetch_block_host,
        fetch_default_deny: config.fetch_default_deny,
        accessible: config.accessible,
        no_emoji: config.no_emoji,
        diag_json: config.diag_json,
        progress: config.progress,
        colour_policy: config.colour_policy,
        spinner_mode: config.spinner_mode,
        output_format: config.output_format,
        theme: canonical_theme(config.theme, config.no_emoji),
        command: Some(resolve_command(parsed.command.as_ref(), &config.cmds.build)),
    }
}

fn resolve_command(parsed: Option<&Commands>, build_defaults: &BuildConfig) -> Commands {
    match parsed {
        Some(Commands::Build(args)) => Commands::Build(BuildArgs {
            emit: args.emit.clone().or_else(|| build_defaults.emit.clone()),
            targets: if args.targets.is_empty() {
                build_defaults.targets.clone()
            } else {
                args.targets.clone()
            },
        }),
        Some(other) => other.clone(),
        None => Commands::Build(BuildArgs {
            emit: build_defaults.emit.clone(),
            targets: build_defaults.targets.clone(),
        }),
    }
}

const fn canonical_theme(theme: Option<Theme>, no_emoji: Option<bool>) -> Option<Theme> {
    match (theme, no_emoji) {
        (Some(value), _) => Some(value),
        (None, Some(true)) => Some(Theme::Ascii),
        _ => None,
    }
}

fn validation_error(key: &str, message: &str) -> Arc<OrthoError> {
    Arc::new(OrthoError::Validation {
        key: key.to_owned(),
        message: message.to_owned(),
    })
}
