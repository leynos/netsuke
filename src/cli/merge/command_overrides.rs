//! Resolve command-specific CLI configuration values during layer composition.

use super::maybe_insert_explicit;
use crate::cli::command::BuildArgs;
#[cfg(feature = "lint")]
use crate::cli::command::{Cli, Commands};
use clap::ArgMatches;
use ortho_config::OrthoResult;
use serde_json::{Map, Value};

/// Collect the `build` subcommand's overrides from explicitly supplied arguments.
///
/// # Errors
///
/// Returns a validation error when a supplied value cannot be serialized.
pub(super) fn build_cli_overrides(
    args: &BuildArgs,
    matches: &ArgMatches,
) -> OrthoResult<Map<String, Value>> {
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
pub(super) fn insert_check_overrides(
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
