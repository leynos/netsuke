//! Layer-composition and conversion helpers for CLI configuration.

use clap::ArgMatches;
use clap::parser::ValueSource;
use ortho_config::declarative::LayerComposition;
use ortho_config::figment::{Figment, providers::Env};
use ortho_config::uncased::Uncased;
use ortho_config::{ConfigDiscovery, MergeComposer, OrthoMergeExt, OrthoResult, sanitize_value};
use ortho_config::{MergeLayer, load_config_file_as_chain};
use serde::Serialize;
use std::borrow::Cow;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Map, Value, json};

use super::config::{BuildConfig, CliConfig, Theme};
use super::parser::{BuildArgs, Cli, Commands};
use super::validation_error;
use crate::theme::ThemePreference;
const CONFIG_ENV_VAR: &str = "NETSUKE_CONFIG";
const CONFIG_ENV_VAR_LEGACY: &str = "NETSUKE_CONFIG_PATH";
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

    push_file_layers(cli, &mut composer, &mut errors);

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
    validate_output_format_source(&merged, matches)?;
    Ok(apply_config(cli, merged))
}

fn push_file_layers(
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

fn env_provider() -> Env {
    Env::prefixed(ENV_PREFIX)
}

fn config_discovery(directory: Option<&PathBuf>) -> ConfigDiscovery {
    let mut builder = ConfigDiscovery::builder("netsuke").env_var(CONFIG_ENV_VAR_LEGACY);
    if let Some(dir) = directory {
        builder = builder.clear_project_roots().add_project_root(dir);
    }
    builder.build()
}

fn collect_file_layers(directory: Option<&Path>) -> OrthoResult<Vec<MergeLayer<'static>>> {
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

fn explicit_config_path(cli: &Cli) -> Option<PathBuf> {
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

fn load_layers_from_path(path: &std::path::Path) -> OrthoResult<Vec<MergeLayer<'static>>> {
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

fn is_empty_value(value: &Value) -> bool {
    matches!(value, Value::Object(map) if map.is_empty())
}

fn diag_json_from_layer(value: &Value) -> Option<bool> {
    value
        .as_object()
        .and_then(|map| map.get("diag_json"))
        .and_then(Value::as_bool)
}

fn collect_diag_file_layers(cli: &Cli) -> OrthoResult<Vec<MergeLayer<'static>>> {
    explicit_config_path(cli).map_or_else(
        || collect_file_layers(cli.directory.as_deref()),
        |path| load_layers_from_path(&path),
    )
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

fn validate_output_format_source(config: &CliConfig, matches: &ArgMatches) -> OrthoResult<()> {
    if matches!(config.output_format, Some(super::OutputFormat::Json))
        && matches.value_source("output_format") != Some(ValueSource::CommandLine)
    {
        return Err(validation_error(
            "output_format",
            "output_format = \"json\" is not supported yet; pass --output-format json explicitly",
        ));
    }
    Ok(())
}

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
    maybe_insert_explicit(matches, "colour_policy", &cli.colour_policy, &mut root)?;
    maybe_insert_explicit(matches, "spinner_mode", &cli.spinner_mode, &mut root)?;
    maybe_insert_explicit(matches, "output_format", &cli.output_format, &mut root)?;
    maybe_insert_explicit(matches, "theme", &cli.theme, &mut root)?;
    maybe_insert_default_targets(cli, matches, &mut root)?;

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

fn maybe_insert_default_targets(
    cli: &Cli,
    matches: &ArgMatches,
    root: &mut Map<String, Value>,
) -> OrthoResult<()> {
    if matches.value_source("default_targets") == Some(ValueSource::CommandLine) {
        root.insert(
            "cmds".to_owned(),
            json!({ "build": { "targets": serialize_value("default_targets", &cli.default_targets)? } }),
        );
    }
    Ok(())
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
    let build_defaults = resolved_build_config(&config);
    Cli {
        file: config.file,
        directory: parsed.directory.clone(),
        config: parsed.config.clone(),
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
        default_targets: build_defaults.targets.clone(),
        command: Some(resolve_command(parsed.command.as_ref(), &build_defaults)),
    }
}

fn resolved_build_config(config: &CliConfig) -> BuildConfig {
    let mut build = config.cmds.build.clone();
    if build.targets.is_empty() {
        build.targets.clone_from(&config.default_targets);
    } else if !config.default_targets.is_empty() {
        let mut targets = config.default_targets.clone();
        targets.extend(build.targets);
        build.targets = targets;
    }
    build
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

fn canonical_theme(theme: Option<Theme>, no_emoji: Option<bool>) -> Option<ThemePreference> {
    match (theme, no_emoji) {
        (Some(value), _) => Some(value.into()),
        (None, Some(true)) => Some(ThemePreference::Ascii),
        _ => None,
    }
}
