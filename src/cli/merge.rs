//! Layer-composition and conversion helpers for CLI configuration.
//!
//! This module bridges the Clap-facing [`Cli`] type from [`super::parser`]
//! and the OrthoConfig-derived [`CliConfig`] schema from [`super::config`].
//! It implements the full four-layer merge pipeline:
//!
//! 1. **Defaults** — `CliConfig::default()` serialised to JSON.
//! 2. **File layers** — discovered and loaded by [`super::discovery`].
//! 3. **Environment layer** — `NETSUKE_`-prefixed variables normalised via
//!    `Uncased` and merged through Figment.
//! 4. **CLI override layer** — fields explicitly supplied on the command line
//!    (as determined by `ArgMatches::value_source`) serialised to JSON.
//!
//! **Pipeline position:** merge layer.
//!
//! - Consumes `(Cli, ArgMatches)` from [`super::parser`].
//! - Applies `CliConfig`'s `PostMergeHook` for cross-field validation.
//! - Produces a fully resolved `Cli` for the runner.
//!
//! Diagnostic JSON resolution lives in [`super::diag`] so it can run before
//! the full merge.

use clap::ArgMatches;
use clap::parser::ValueSource;
use ortho_config::declarative::LayerComposition;
use ortho_config::figment::Figment;
use ortho_config::{MergeComposer, OrthoMergeExt, OrthoResult, sanitize_value};
use serde::Serialize;

use serde_json::{Map, Value, json};

use super::config::{BuildConfig, CliConfig};
use super::discovery::{
    DiscoveredLayers, EnvProvider, StdEnvProvider, discover_file_layers,
    push_discovered_file_layers,
};
use super::environment::EnvironmentLayer;
use super::parser::{BuildArgs, Cli, Commands};
use super::validation_error;

/// Merge discovered configuration layers over parsed CLI input.
///
/// # Errors
///
/// Returns an [`ortho_config::OrthoError`] if layer composition or merging
/// fails.
pub fn merge_with_config(cli: &Cli, matches: &ArgMatches) -> OrthoResult<Cli> {
    merge_with_config_and_env(cli, matches, &StdEnvProvider)
}

/// Merge configuration layers using an explicit environment provider.
///
/// This is the deterministic boundary for tests and adapters that must supply
/// both configuration selectors and `NETSUKE_*` values without mutating the
/// process environment.
///
/// # Errors
///
/// Returns an [`ortho_config::OrthoError`] if layer composition or merging
/// fails.
pub fn merge_with_config_and_env(
    cli: &Cli,
    matches: &ArgMatches,
    env: &impl EnvProvider,
) -> OrthoResult<Cli> {
    let outcome = discover_file_layers(cli, env);
    outcome.emit_diagnostics();
    merge_with_cached_file_layers(cli, matches, env, outcome.into_layers())
}

/// Merge configuration using file layers discovered by an earlier phase.
///
/// This composition boundary is used after diagnostic-mode resolution so the
/// full merge reuses the first discovery pass rather than loading files again.
///
/// # Errors
///
/// Returns an [`ortho_config::OrthoError`] if layer composition or merging
/// fails.
pub fn merge_with_cached_file_layers(
    cli: &Cli,
    matches: &ArgMatches,
    env: &impl EnvProvider,
    discovered: DiscoveredLayers,
) -> OrthoResult<Cli> {
    let mut errors = Vec::new();
    let mut composer = MergeComposer::with_capacity(4);

    push_defaults_layer(&mut composer, &mut errors);
    push_discovered_file_layers(&mut composer, &mut errors, discovered);
    push_environment_layer(env, &mut composer, &mut errors);
    push_cli_layer(cli, matches, &mut composer, &mut errors);

    let composition = LayerComposition::new(composer.layers(), errors);
    let merged = composition.into_merge_result(CliConfig::merge_from_layers)?;
    Ok(apply_config(cli, merged))
}

/// Push the default configuration layer and retain any serialization failure.
///
/// This helper belongs only to the cached merge boundary: it must append to the
/// caller's shared composer and error collection rather than finish a merge.
fn push_defaults_layer(
    composer: &mut MergeComposer,
    errors: &mut Vec<std::sync::Arc<ortho_config::OrthoError>>,
) {
    match sanitize_value(&CliConfig::default()) {
        Ok(value) => {
            tracing::debug!(layer = "defaults", "applied default configuration layer");
            composer.push_defaults(value);
        }
        Err(err) => {
            tracing::debug!(layer = "defaults", "default configuration layer failed");
            errors.push(err);
        }
    }
}

/// Push the injected environment layer and retain extraction failures.
///
/// This helper belongs only to the cached merge boundary and never reads the
/// process environment: callers supply the environment adapter explicitly.
fn push_environment_layer(
    env: &impl EnvProvider,
    composer: &mut MergeComposer,
    errors: &mut Vec<std::sync::Arc<ortho_config::OrthoError>>,
) {
    match Figment::from(EnvironmentLayer::new(env.entries()))
        .extract::<Value>()
        .into_ortho_merge()
    {
        Ok(value) => {
            tracing::debug!(
                layer = "environment",
                is_empty = is_empty_value(&value),
                "merged environment configuration layer"
            );
            composer.push_environment(value);
        }
        Err(err) => {
            tracing::debug!(
                layer = "environment",
                "environment configuration layer failed"
            );
            errors.push(err);
        }
    }
}

/// Push explicit CLI overrides and log their keys without recording values.
///
/// This helper is limited to the cached merge boundary because only that
/// boundary owns the shared layer order and accumulated error collection.
fn push_cli_layer(
    cli: &Cli,
    matches: &ArgMatches,
    composer: &mut MergeComposer,
    errors: &mut Vec<std::sync::Arc<ortho_config::OrthoError>>,
) {
    match cli_overrides_from_matches(cli, matches) {
        Ok(value) if !is_empty_value(&value) => {
            // Values may echo user-supplied paths or host lists, so records
            // identify only the keys that were explicitly overridden.
            let override_keys = value
                .as_object()
                .map(|map| map.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            tracing::debug!(
                layer = "cli",
                override_keys = ?override_keys,
                "applied CLI override layer"
            );
            composer.push_cli(value);
        }
        Ok(_) => tracing::debug!(layer = "cli", "no explicit CLI overrides supplied"),
        Err(err) => {
            tracing::debug!(layer = "cli", "CLI override layer failed");
            errors.push(err);
        }
    }
}

fn is_empty_value(value: &Value) -> bool {
    matches!(value, Value::Object(map) if map.is_empty())
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
    maybe_insert_explicit(matches, "json", &cli.json, &mut root)?;
    maybe_insert_explicit(matches, "no_input", &cli.no_input(), &mut root)?;
    maybe_insert_explicit(matches, "color", &cli.color, &mut root)?;
    maybe_insert_explicit(matches, "emoji", &cli.emoji, &mut root)?;
    maybe_insert_explicit(matches, "progress", &cli.progress, &mut root)?;
    maybe_insert_explicit(matches, "accessibility", &cli.accessibility, &mut root)?;

    let mut cmds_build: Map<String, Value> = Map::new();

    if matches.value_source("default_targets") == Some(ValueSource::CommandLine) {
        cmds_build.insert(
            "targets".to_owned(),
            serialize_value("default_targets", &cli.default_targets)?,
        );
    }

    if let Some(Commands::Build(args)) = cli.command.as_ref()
        && let Some(build_matches) = matches.subcommand_matches("build")
    {
        for (k, v) in build_cli_overrides(args, build_matches)? {
            cmds_build.insert(k, v);
        }
    }

    if !cmds_build.is_empty() {
        root.insert(
            "cmds".to_owned(),
            json!({ "build": Value::Object(cmds_build) }),
        );
    }

    Ok(Value::Object(root))
}

fn build_cli_overrides(args: &BuildArgs, matches: &ArgMatches) -> OrthoResult<Map<String, Value>> {
    let mut build = Map::new();
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
        json: config.json,
        interaction: super::parser::InteractionArgs {
            no_input: config.no_input.is_enabled(),
        },
        color: config.color,
        emoji: config.emoji,
        progress: config.progress,
        accessibility: config.accessibility,
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
            targets: if args.targets.is_empty() {
                build_defaults.targets.clone()
            } else {
                args.targets.clone()
            },
        }),
        Some(other) => other.clone(),
        None => Commands::Build(BuildArgs {
            targets: build_defaults.targets.clone(),
        }),
    }
}
