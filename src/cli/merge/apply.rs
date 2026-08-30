//! Conversion from validated configuration into the runtime CLI shape.
//!
//! Keeps post-merge command/default resolution separate from layer collection
//! so merge orchestration remains compact and independently understandable.

use super::super::command::{BuildArgs, CheckArgs, Cli, Commands, InteractionArgs};
use super::super::config::{BuildConfig, CheckConfig, CliConfig};

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
        env_allow_var: config.env_allow_var,
        env_block_var: config.env_block_var,
        fetch_allow_host: config.fetch_allow_host,
        fetch_block_host: config.fetch_block_host,
        fetch_default_deny: config.fetch_default_deny,
        trust_project_fetch_policy: config.trust_project_fetch_policy,
        manifest_evaluation_fuel: config.manifest_evaluation_fuel,
        manifest_fuel: config.manifest_fuel,
        manifest_rendered_value_bytes: config.manifest_rendered_value_bytes,
        manifest_rendered_manifest_bytes: config.manifest_rendered_manifest_bytes,
        manifest_source_bytes: config.manifest_source_bytes,
        manifest_foreach_cardinality: config.manifest_foreach_cardinality,
        manifest_expanded_entries: config.manifest_expanded_entries,
        json: config.json,
        interaction: InteractionArgs {
            no_input: config.no_input.is_enabled(),
        },
        color: config.color,
        emoji: config.emoji,
        progress: config.progress,
        accessibility: config.accessibility,
        default_targets: build_defaults.targets.clone(),
        command: Some(resolve_command(
            parsed.command.as_ref(),
            &build_defaults,
            &config.cmds.check,
        )),
    }
}

/// Resolve effective build defaults from root and subcommand target settings.
fn resolved_build_config(config: &CliConfig) -> BuildConfig {
    let mut build = config.cmds.build.clone();
    let mut targets = config.default_targets.clone();
    targets.extend(build.targets);
    build.targets = targets;
    build
}

/// Resolve the final command, substituting default targets when none were given.
fn resolve_command(
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
            .filter(|_| args.fail_on == super::DEFAULT_FAIL_ON)
            .unwrap_or_else(|| args.fail_on.clone()),
        limit: config
            .limit
            .filter(|_| args.limit == super::DEFAULT_FINDING_LIMIT)
            .unwrap_or(args.limit),
        explain: args.explain.clone(),
    }
}
