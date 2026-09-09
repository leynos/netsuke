//! Conversion from validated configuration into the runtime CLI shape.
//!
//! Keeps post-merge command/default resolution separate from layer collection
//! so merge orchestration remains compact and independently understandable.

use super::command::{BuildArgs, Cli, Commands, InteractionArgs};
use super::config::{BuildConfig, CliConfig};

/// Apply merged configuration over parsed CLI input to build the runtime CLI.
pub(super) fn apply_config(parsed: &Cli, config: CliConfig) -> Cli {
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
        trust_project_fetch_policy: config.trust_project_fetch_policy,
        json: config.json,
        interaction: InteractionArgs {
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

/// Resolve effective build defaults from root and subcommand target settings.
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
