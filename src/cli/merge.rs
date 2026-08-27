//! Layer-composition and conversion helpers for CLI configuration.
//!
//! This module bridges the Clap-facing [`Cli`] type from [`super::parser`]
//! and the OrthoConfig-derived [`CliConfig`] schema from [`super::config`].
//! It implements the full four-layer merge pipeline:
//!
//! 1. **Defaults** — `CliConfig::default()` serialized to JSON.
//! 2. **File layers** — discovered and loaded by [`super::discovery`].
//! 3. **Environment layer** — `NETSUKE_`-prefixed variables normalized via
//!    `Uncased` and merged through Figment.
//! 4. **CLI override layer** — fields explicitly supplied on the command line
//!    (as determined by `ArgMatches::value_source`) serialized to JSON.
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
use ortho_config::figment::Figment;
use ortho_config::{OrthoError, OrthoMergeExt, OrthoResult, sanitize_value};
use serde::Serialize;

use serde_json::{Map, Value, json};

use super::config::{BuildConfig, CliConfig, validation_rejection_reason};
use super::discovery::{
    DiscoveredLayers, EnvProvider, StdEnvProvider, discover_file_layers,
    push_discovered_file_layers,
};
use super::environment::EnvironmentLayer;
use super::merge_input::{CachedMergeInput, MergeComposition};
use super::merge_observability::{
    NoopMergeObserver, collect_override_leaf_paths, is_empty_configuration_value,
};
use super::parser::{BuildArgs, Cli, Commands};
use super::validation_error;
use super::{MergeEvent, MergeObserver};

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
    let mut observer = NoopMergeObserver;
    let input = CachedMergeInput::new(cli, matches, env, discovered);
    merge_with_cached_file_layers_with_observer(input, &mut observer)
}

/// Merge cached configuration layers and report bounded events to `observer`.
///
/// Application adapters opt into observability by supplying an observer. The
/// ordinary merge functions use a no-op observer so they remain side-effect
/// free and reusable in non-CLI contexts.
///
/// # Errors
///
/// Returns an [`ortho_config::OrthoError`] if layer composition or merging
/// fails.
pub fn merge_with_cached_file_layers_with_observer<E, O>(
    input: CachedMergeInput<'_, E>,
    observer: &mut O,
) -> OrthoResult<Cli>
where
    E: EnvProvider + ?Sized,
    O: MergeObserver + ?Sized,
{
    let CachedMergeInput {
        cli,
        matches,
        env,
        discovered,
    } = input;
    let mut composition = MergeComposition::new();

    push_defaults_layer(&mut composition, observer);
    push_discovered_file_layers(
        &mut composition.composer,
        &mut composition.errors,
        discovered,
        observer,
    );
    push_environment_layer(env, &mut composition, observer);
    push_cli_layer(cli, matches, &mut composition, observer);

    let merged = composition
        .into_merge_result()
        .inspect_err(|error| observe_validation_rejection(observer, error.as_ref()))?;
    Ok(apply_config(cli, merged))
}

/// Push the default configuration layer and retain any serialization failure.
///
/// This helper belongs only to the cached merge boundary: it must append to the
/// caller's shared composer and error collection rather than finish a merge.
fn push_defaults_layer<O>(composition: &mut MergeComposition, observer: &mut O)
where
    O: MergeObserver + ?Sized,
{
    match sanitize_value(&CliConfig::default()) {
        Ok(value) => {
            observer.observe(MergeEvent::DefaultsApplied);
            composition.composer.push_defaults(value);
        }
        Err(err) => {
            observer.observe(MergeEvent::DefaultsFailed);
            composition.errors.push(err);
        }
    }
}

/// Push the injected environment layer and retain extraction failures.
///
/// This helper belongs only to the cached merge boundary and never reads the
/// process environment: callers supply the environment adapter explicitly.
fn push_environment_layer<O>(
    env: &(impl EnvProvider + ?Sized),
    composition: &mut MergeComposition,
    observer: &mut O,
) where
    O: MergeObserver + ?Sized,
{
    match Figment::from(EnvironmentLayer::new(env.entries()))
        .extract::<Value>()
        .into_ortho_merge()
    {
        Ok(value) => {
            observer.observe(MergeEvent::EnvironmentApplied {
                is_empty: is_empty_configuration_value(&value),
            });
            composition.composer.push_environment(value);
        }
        Err(err) => {
            observer.observe(MergeEvent::EnvironmentFailed);
            composition.errors.push(err);
        }
    }
}

/// Push explicit CLI overrides and log their keys without recording values.
///
/// This helper is limited to the cached merge boundary because only that
/// boundary owns the shared layer order and accumulated error collection.
fn push_cli_layer<O>(
    cli: &Cli,
    matches: &ArgMatches,
    composition: &mut MergeComposition,
    observer: &mut O,
) where
    O: MergeObserver + ?Sized,
{
    match cli_overrides_from_matches(cli, matches) {
        Ok(value) if !is_empty_configuration_value(&value) => {
            // Values may echo user-supplied paths or host lists, so records
            // identify only the keys that were explicitly overridden.
            observer.observe(MergeEvent::CliOverridesApplied {
                override_keys: collect_override_leaf_paths(&value),
            });
            composition.composer.push_cli(value);
        }
        Ok(_) => observer.observe(MergeEvent::CliOverridesAbsent),
        Err(err) => {
            observer.observe(MergeEvent::CliOverridesFailed);
            composition.errors.push(err);
        }
    }
}
/// Forward known validation rejections to the explicit observer with fixed data.
fn observe_validation_rejection<O>(observer: &mut O, error: &OrthoError)
where
    O: MergeObserver + ?Sized,
{
    if let OrthoError::Validation { key, .. } = error
        && let Some(reason) = validation_rejection_reason(key)
    {
        observer.observe(MergeEvent::ValidationRejected {
            key: key.clone(),
            reason,
        });
    }
}

/// Collect CLI-layer overrides from arguments explicitly supplied on the command line.
///
/// # Errors
///
/// Returns a validation error when a supplied value cannot be serialized.
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

/// Collect the `build` subcommand's overrides from explicitly supplied arguments.
///
/// # Errors
///
/// Returns a validation error when a supplied value cannot be serialized.
fn build_cli_overrides(args: &BuildArgs, matches: &ArgMatches) -> OrthoResult<Map<String, Value>> {
    let mut build = Map::new();
    maybe_insert_explicit(matches, "targets", &args.targets, &mut build)?;
    Ok(build)
}

/// Insert `field` into `target` when `matches` reports it was supplied on the
/// command line.
///
/// # Errors
///
/// Returns a validation error when `value` cannot be serialized.
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

/// Serialize `value` for the named configuration field.
///
/// # Errors
///
/// Returns a validation error when serialization fails.
fn serialize_value<T>(field: &str, value: &T) -> OrthoResult<Value>
where
    T: Serialize,
{
    serde_json::to_value(value).map_err(|err| validation_error(field, &err.to_string()))
}

/// Apply the merged configuration over the parsed CLI input, producing the
/// resolved runtime `Cli`.
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

/// Resolve the effective build defaults, combining root-level default targets
/// with subcommand-level targets.
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

/// Resolve the final command, substituting default targets when none were given.
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
