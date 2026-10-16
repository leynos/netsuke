//! Resolve command-specific CLI configuration values during layer composition.

use super::{maybe_insert_explicit, serialize_value};
use crate::cli::command::{BuildArgs, Cli, Commands};
use clap::ArgMatches;
use clap::parser::ValueSource;
use ortho_config::OrthoResult;
use serde_json::{Map, Value};

/// Collect every subcommand's explicitly supplied arguments, keyed by command.
///
/// The result is the `cmds` table: `build` gathers the global
/// `default_targets` option alongside `build`'s own arguments, and `check`
/// appears only when the linter is compiled in. A command with no explicit
/// argument is omitted, so an empty map means "no command-line override".
///
/// # Errors
///
/// Returns a validation error when a supplied value cannot be serialized.
pub(super) fn command_cli_overrides(
    cli: &Cli,
    matches: &ArgMatches,
) -> OrthoResult<Map<String, Value>> {
    let mut cmds = Map::new();
    let build = build_command_overrides(cli, matches)?;
    if !build.is_empty() {
        cmds.insert("build".to_owned(), Value::Object(build));
    }
    #[cfg(feature = "lint")]
    insert_check_overrides(cli, matches, &mut cmds)?;
    Ok(cmds)
}

/// Collect the `cmds.build` table from `default_targets` and the `build`
/// subcommand's own arguments.
///
/// # Errors
///
/// Returns a validation error when a supplied value cannot be serialized.
fn build_command_overrides(cli: &Cli, matches: &ArgMatches) -> OrthoResult<Map<String, Value>> {
    let mut cmds_build = Map::new();
    if matches.value_source("default_targets") == Some(ValueSource::CommandLine) {
        cmds_build.insert(
            "targets".to_owned(),
            serialize_value("default_targets", &cli.default_targets)?,
        );
    }
    if let Some(Commands::Build(args)) = cli.command.as_ref()
        && let Some(build_matches) = matches.subcommand_matches("build")
    {
        cmds_build.extend(build_cli_overrides(args, build_matches)?);
    }
    Ok(cmds_build)
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

/// Insert the `check` subcommand's explicit arguments under `cmds.check`.
///
/// Nothing is inserted when no `check` argument was supplied, so an absent
/// `check` table still means "no command-line override".
///
/// # Errors
///
/// Returns a validation error when a supplied value cannot be serialized.
#[cfg(feature = "lint")]
fn insert_check_overrides(
    cli: &Cli,
    matches: &ArgMatches,
    cmds: &mut Map<String, Value>,
) -> OrthoResult<()> {
    let check = check_overrides(cli, matches)?;
    if !check.is_empty() {
        cmds.insert("check".to_owned(), Value::Object(check));
    }
    Ok(())
}

/// Collect the `check` subcommand's explicitly supplied arguments.
///
/// # Errors
///
/// Returns a validation error when a supplied value cannot be serialized.
#[cfg(feature = "lint")]
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
