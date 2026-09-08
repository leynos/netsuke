//! Resolve command-specific CLI configuration values during layer composition.

use super::maybe_insert_explicit;
use crate::cli::{
    command::{BuildArgs, Commands},
    config::BuildConfig,
};
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

/// Resolve the effective build defaults, combining root-level and subcommand targets.
pub(super) fn resolved_build_config(config: &crate::cli::config::CliConfig) -> BuildConfig {
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
pub(super) fn resolve_command(parsed: Option<&Commands>, build_defaults: &BuildConfig) -> Commands {
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
