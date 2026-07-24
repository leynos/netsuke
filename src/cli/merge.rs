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
use ortho_config::figment::{Figment, providers::Env};
use ortho_config::uncased::Uncased;
use ortho_config::{MergeComposer, OrthoMergeExt, OrthoResult, sanitize_value};
use serde::Serialize;

use serde_json::{Map, Value, json};

use super::config::{BuildConfig, CliConfig};
use super::discovery::push_file_layers;
use super::parser::{BuildArgs, Cli, Commands};
use super::validation_error;

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
    Ok(apply_config(cli, merged))
}

pub(crate) fn env_provider() -> Env {
    Env::prefixed(ENV_PREFIX)
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
