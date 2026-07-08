//! Step definitions for CLI parsing scenarios.
//!
//! Provides BDD step functions for parsing command-line arguments via `clap`,
//! verifying parsed commands, and checking CLI network policy behaviour.
//! Steps store results in [`TestWorld`] for downstream assertions.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::helpers::parse_store::store_parse_outcome;
use crate::bdd::helpers::tokens::build_tokens;
use crate::bdd::types::{CliArgs, ErrorFragment, JobCount, PathString, TargetName, UrlString};
use anyhow::{Context, Result, bail};
use netsuke::cli::{Cli, Commands};
use netsuke::cli_localization;
use netsuke::locale_resolution;
use rstest_bdd_macros::then;
use std::path::PathBuf;
use std::sync::Arc;
use test_support::locale_stubs::{StubEnv, StubSystemLocale};

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Apply CLI parsing, storing result or error in world state.
///
/// This function always runs `merge_with_config`, which performs automatic
/// configuration discovery and environment variable merging. To ensure tests
/// remain hermetic and isolated from the host environment, callers should:
///
/// 1. Set `NETSUKE_CONFIG_PATH` to an empty/nonexistent path to disable config
///    file discovery, or ensure a `temp_dir` is set to anchor discovery to a
///    controlled location.
/// 2. Clear or set all `NETSUKE_*` environment variables to known values.
///
/// Tests that do not explicitly set up configuration or environment variables
/// may be affected by ambient host configuration.
pub(super) fn apply_cli(world: &TestWorld, args: &CliArgs) {
    let env = StubEnv {
        locale: world.locale_env.get(),
    };
    let system = StubSystemLocale {
        locale: world.locale_system.get(),
    };

    // If there's a temp_dir set and the args don't already contain an
    // explicit -C or --directory flag, prepend -C <temp_dir> for config discovery.
    let mut tokens = build_tokens(args.as_str());
    if let Some(temp_dir) = world.temp_dir.borrow().as_ref() {
        let is_directory_flag = |t: &std::ffi::OsString| {
            t.to_str().is_some_and(|s| {
                s == "-C"
                    || s.starts_with("-C")
                    || s == "--directory"
                    || s.starts_with("--directory=")
            })
        };
        let has_directory_flag = tokens.iter().any(is_directory_flag);
        if !has_directory_flag && !tokens.is_empty() {
            let temp_path = temp_dir.path().as_os_str().to_owned();
            tokens.insert(1, "-C".into());
            tokens.insert(2, temp_path);
        }
    }

    let locale = locale_resolution::resolve_startup_locale(&tokens, &env, &system);
    let localizer = Arc::from(cli_localization::build_localizer(locale.as_deref()));
    let outcome = netsuke::cli::parse_with_localizer_from(tokens, &localizer)
        .map_err(|e| e.to_string())
        .and_then(|(parsed_cli, matches)| {
            // Apply config file discovery and merge
            netsuke::cli::merge_with_config(&parsed_cli, &matches)
                .map(normalize_cli)
                .map_err(|e| e.to_string())
        });
    store_parse_outcome(&world.cli, &world.cli_error, outcome);
}

/// Get the CLI's network policy.
fn cli_network_policy(world: &TestWorld) -> Result<netsuke::stdlib::NetworkPolicy> {
    world
        .cli
        .with_ref(Cli::network_policy)
        .context("CLI has not been parsed")?
        .context("construct CLI network policy")
}

/// Extract build command args (targets and emit path).
fn extract_build(world: &TestWorld) -> Result<(Vec<String>, Option<PathBuf>)> {
    world
        .cli
        .with_ref(|cli| {
            let command = cli.command.as_ref()?;
            match command {
                Commands::Build(args) => Some((args.targets.clone(), args.emit.clone())),
                _ => None,
            }
        })
        .flatten()
        .context("expected build command")
}

/// Extract graph command args.
fn extract_graph_args(world: &TestWorld) -> Result<netsuke::cli::GraphArgs> {
    match get_command(world)? {
        Commands::Graph(args) => Ok(args),
        other => bail!("expected graph command, got {other:?}"),
    }
}

/// Extract manifest command file path.
fn extract_manifest_command_file(world: &TestWorld) -> Result<PathBuf> {
    match get_command(world)? {
        Commands::Manifest { file } => Ok(file),
        other => bail!("expected manifest command, got {other:?}"),
    }
}

/// Get the parsed CLI command.
fn get_command(world: &TestWorld) -> Result<Commands> {
    world
        .cli
        .with_ref(|cli| cli.command.clone())
        .context("CLI has not been parsed")?
        .context("CLI command missing")
}

// ---------------------------------------------------------------------------
// CLI parsing helpers
// ---------------------------------------------------------------------------

/// Normalise a parsed CLI by setting default command if missing.
fn normalize_cli(cli: Cli) -> Cli {
    cli.with_default_command()
}

#[path = "cli_verify.rs"]
mod cli_verify;
use cli_verify::{
    ExpectedCommand, verify_cli_policy_allows, verify_cli_policy_rejects, verify_command,
    verify_emit_path, verify_error_contains, verify_error_returned, verify_first_target,
    verify_graph_html_set, verify_graph_output_path, verify_job_count,
    verify_manifest_command_path, verify_manifest_path, verify_parsing_succeeded,
    verify_working_directory,
};

// ---------------------------------------------------------------------------
// Given/When steps
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then]
fn parsing_succeeds(world: &TestWorld) -> Result<()> {
    verify_parsing_succeeded(world)
}

#[then]
fn the_command_is_build(world: &TestWorld) -> Result<()> {
    verify_command(world, ExpectedCommand::Build)
}

#[then]
fn the_command_is_clean(world: &TestWorld) -> Result<()> {
    verify_command(world, ExpectedCommand::Clean)
}

#[then]
fn the_command_is_graph(world: &TestWorld) -> Result<()> {
    verify_command(world, ExpectedCommand::Graph)
}

#[then]
fn the_command_is_manifest(world: &TestWorld) -> Result<()> {
    verify_command(world, ExpectedCommand::Manifest)
}

#[then("the manifest path is {path:string}")]
fn manifest_path(world: &TestWorld, path: PathString) -> Result<()> {
    verify_manifest_path(world, &path)
}

#[then("the first target is {target:string}")]
fn first_target(world: &TestWorld, target: TargetName) -> Result<()> {
    verify_first_target(world, &target)
}

#[then("the working directory is {directory:string}")]
fn working_directory(world: &TestWorld, directory: PathString) -> Result<()> {
    verify_working_directory(world, &directory)
}

#[then("the job count is {count:usize}")]
fn job_count(world: &TestWorld, count: usize) -> Result<()> {
    verify_job_count(world, JobCount::new(count))
}

#[then("the emit path is {path:string}")]
fn emit_path(world: &TestWorld, path: PathString) -> Result<()> {
    verify_emit_path(world, &path)
}

#[then("the CLI network policy allows {url:string}")]
fn cli_policy_allows(world: &TestWorld, url: UrlString) -> Result<()> {
    verify_cli_policy_allows(world, &url)
}

#[then("the CLI network policy rejects {url:string} with {message:string}")]
fn cli_policy_rejects(world: &TestWorld, url: UrlString, message: ErrorFragment) -> Result<()> {
    verify_cli_policy_rejects(world, &url, &message)
}

#[then("the manifest command path is {path:string}")]
fn manifest_command_path(world: &TestWorld, path: PathString) -> Result<()> {
    verify_manifest_command_path(world, &path)
}

#[then("the graph output path is {path:string}")]
fn graph_output_path(world: &TestWorld, path: PathString) -> Result<()> {
    verify_graph_output_path(world, &path)
}

#[then("the graph html flag is set")]
fn graph_html_flag_is_set(world: &TestWorld) -> Result<()> {
    verify_graph_html_set(world)
}

#[then]
fn an_error_should_be_returned(world: &TestWorld) -> Result<()> {
    verify_error_returned(world)
}

#[then("the error message should contain {fragment:string}")]
fn error_message_should_contain(world: &TestWorld, fragment: ErrorFragment) -> Result<()> {
    verify_error_contains(world, &fragment)
}
