//! Resolve command-specific CLI configuration values during layer composition.

use super::maybe_insert_explicit;
use crate::cli::command::BuildArgs;
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
