//! Layering of subcommand-scoped configuration under the `cmds` namespace.
//!
//! Root flags and subcommand flags travel through the same composer but answer
//! different questions: a root flag configures Netsuke, whereas a subcommand
//! flag configures one command's defaults and must not leak into another's.
//! Keeping that distinction in its own module is what stops the two sets of
//! rules being read as one.

use clap::ArgMatches;
use clap::parser::ValueSource;
use ortho_config::OrthoResult;
use serde_json::{Map, Value};

use super::command::{BuildArgs, CheckArgs, Cli, Commands, DEFAULT_FAIL_ON, DEFAULT_FINDING_LIMIT};
use super::config::{BuildConfig, CheckConfig, CliConfig};
use super::merge::{maybe_insert_explicit, serialize_value};

/// Collect every subcommand's explicitly supplied arguments, keyed by command.
///
/// # Errors
///
/// Returns a validation error when a supplied value cannot be serialized.
pub(super) fn overrides(cli: &Cli, matches: &ArgMatches) -> OrthoResult<Map<String, Value>> {
    let mut cmds = Map::new();
    for (name, collected) in [
        ("build", build_overrides(cli, matches)?),
        ("check", check_overrides(cli, matches)?),
    ] {
        if !collected.is_empty() {
            cmds.insert(name.to_owned(), Value::Object(collected));
        }
    }
    Ok(cmds)
}

/// Collect the `build` subcommand's overrides, including the root-level
/// `--default-target` flag that feeds the same defaults.
///
/// # Errors
///
/// Returns a validation error when a supplied value cannot be serialized.
fn build_overrides(cli: &Cli, matches: &ArgMatches) -> OrthoResult<Map<String, Value>> {
    let mut build = Map::new();
    if matches.value_source("default_targets") == Some(ValueSource::CommandLine) {
        build.insert(
            "targets".to_owned(),
            serialize_value("default_targets", &cli.default_targets)?,
        );
    }
    if let Some(Commands::Build(args)) = cli.command.as_ref()
        && let Some(build_matches) = matches.subcommand_matches("build")
    {
        build.extend(build_cli_overrides(args, build_matches)?);
    }
    Ok(build)
}

/// Collect the `check` subcommand's explicitly supplied arguments.
///
/// # Errors
///
/// Returns a validation error when a supplied value cannot be serialized.
fn check_overrides(cli: &Cli, matches: &ArgMatches) -> OrthoResult<Map<String, Value>> {
    let mut check = Map::new();
    let Some(Commands::Check(args)) = cli.command.as_ref() else {
        return Ok(check);
    };
    let Some(check_matches) = matches.subcommand_matches("check") else {
        return Ok(check);
    };
    maybe_insert_explicit(check_matches, "rule", &args.rule, &mut check)?;
    maybe_insert_explicit(check_matches, "fail_on", &args.fail_on, &mut check)?;
    maybe_insert_explicit(check_matches, "limit", &args.limit, &mut check)?;
    Ok(check)
}

/// Resolve the effective `check` arguments from the CLI and configuration.
///
/// Command-line values already won the merge, so the configuration only fills
/// in the fields the caller left at their defaults.
fn resolve_check_args(args: &CheckArgs, config: &CheckConfig) -> CheckArgs {
    CheckArgs {
        rule: if args.rule.is_empty() {
            config.rule.clone()
        } else {
            args.rule.clone()
        },
        fail_on: config
            .fail_on
            .clone()
            .filter(|_| args.fail_on == DEFAULT_FAIL_ON)
            .unwrap_or_else(|| args.fail_on.clone()),
        limit: config
            .limit
            .filter(|_| args.limit == DEFAULT_FINDING_LIMIT)
            .unwrap_or(args.limit),
        explain: args.explain.clone(),
    }
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

/// Resolve the effective build defaults, combining root-level default targets
/// with subcommand-level targets.
pub(super) fn resolved_build_config(config: &CliConfig) -> BuildConfig {
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
pub(super) fn resolve_command(
    parsed: Option<&Commands>,
    build_defaults: &BuildConfig,
    check_defaults: &CheckConfig,
) -> Commands {
    match parsed {
        Some(Commands::Build(args)) => Commands::Build(BuildArgs {
            targets: if args.targets.is_empty() {
                build_defaults.targets.clone()
            } else {
                args.targets.clone()
            },
        }),
        Some(Commands::Check(args)) => Commands::Check(resolve_check_args(args, check_defaults)),
        Some(other) => other.clone(),
        None => Commands::Build(BuildArgs {
            targets: build_defaults.targets.clone(),
        }),
    }
}
