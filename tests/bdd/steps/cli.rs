//! Step definitions for CLI parsing scenarios.
//!
//! Provides BDD step functions for parsing command-line arguments via `clap`,
//! verifying parsed commands, and checking CLI network policy behaviour.
//! Steps store results in [`TestWorld`] for downstream assertions.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::helpers::config_environment::merge_with_world_env;
use crate::bdd::helpers::parse_store::store_parse_outcome;
use crate::bdd::helpers::tokens::build_tokens;
use crate::bdd::types::{CliArgs, ErrorFragment, JobCount, PathString, TargetName, UrlString};
use anyhow::{Context, Result, bail};
use netsuke::cli::{Cli, Commands, HelpTopic};
use netsuke::cli_localization;
use netsuke::locale_resolution;
use rstest_bdd_macros::then;
use std::ffi::{OsStr, OsString};
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
/// 1. Ensure a `temp_dir` is set to anchor discovery to a controlled location.
/// 2. Clear or set all `NETSUKE_*` environment variables to known values.
///
/// Tests that do not explicitly set up configuration or environment variables
/// may be affected by ambient host configuration.
pub(super) fn apply_cli(world: &TestWorld, args: &CliArgs) {
    apply_cli_tokens(world, build_tokens(args.as_str()));
}

/// Apply parsed CLI argument tokens, storing the result or error in world state.
///
/// This accepts fully formed arguments for scenarios whose temporary-resource
/// paths cannot be represented as static feature text.
pub(super) fn apply_cli_tokens(world: &TestWorld, mut tokens: Vec<std::ffi::OsString>) {
    let env = world
        .locale_env
        .get()
        .map_or_else(StubEnv::without_locale, StubEnv::with_locale);
    let system = StubSystemLocale {
        locale: world.locale_system.get(),
    };

    // If there's a temp_dir set and the args don't already contain an
    // explicit -C or --directory flag, prepend -C <temp_dir> for config discovery.
    if let Some(temp_dir) = world.temp_dir.borrow().as_ref() {
        insert_discovery_directory_if_missing(&mut tokens, temp_dir.path().as_os_str());
    }

    let locale = locale_resolution::resolve_startup_locale(&tokens, &env, &system);
    let localizer = Arc::from(cli_localization::build_localizer(locale.as_deref()));
    let outcome = netsuke::cli::parse_with_localizer_from(tokens, &localizer)
        .map_err(|e| e.to_string())
        .and_then(|(parsed_cli, matches)| {
            // Apply config file discovery and merge
            merge_with_world_env(world, &parsed_cli, &matches)
                .map(normalize_cli)
                .map_err(|e| e.to_string())
        });
    store_parse_outcome(&world.cli, &world.cli_error, outcome);
}

/// Add the BDD discovery anchor unless the scenario already supplies one.
///
/// This helper belongs only to the BDD CLI step boundary. Keeping detection on
/// encoded bytes preserves attached directory values even when they are not
/// valid UTF-8, so `apply_cli_tokens` never adds a competing directory option.
fn insert_discovery_directory_if_missing(tokens: &mut Vec<OsString>, directory: &OsStr) {
    let has_directory_flag = tokens
        .iter()
        .any(|token| is_directory_flag(token.as_os_str()));
    if !has_directory_flag && !tokens.is_empty() {
        tokens.insert(1, "-C".into());
        tokens.insert(2, directory.to_owned());
    }
}

/// Return whether a token is a standalone or attached directory option.
fn is_directory_flag(token: &OsStr) -> bool {
    let bytes = token.as_encoded_bytes();
    bytes == b"-C"
        || bytes.starts_with(b"-C")
        || bytes == b"--directory"
        || bytes.starts_with(b"--directory=")
}

/// Get the CLI's network policy.
fn cli_network_policy(world: &TestWorld) -> Result<netsuke::stdlib::NetworkPolicy> {
    world
        .cli
        .with_ref(Cli::network_policy)
        .context("CLI has not been parsed")?
        .context("construct CLI network policy")
}

/// Extract build command targets.
fn extract_build(world: &TestWorld) -> Result<Vec<String>> {
    world
        .cli
        .with_ref(|cli| {
            let command = cli.command.as_ref()?;
            match command {
                Commands::Build(args) => Some(args.targets.clone()),
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

/// Extract the optional generate output path.
fn extract_generate_output(world: &TestWorld) -> Result<Option<PathBuf>> {
    match get_command(world)? {
        Commands::Generate { output } => Ok(output),
        other => bail!("expected generate command, got {other:?}"),
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
    verify_error_contains, verify_error_returned, verify_first_target, verify_generate_output_path,
    verify_graph_html_set, verify_graph_output_path, verify_help_topic, verify_job_count,
    verify_manifest_path, verify_parsing_succeeded, verify_working_directory,
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
fn the_command_is_generate(world: &TestWorld) -> Result<()> {
    verify_command(world, ExpectedCommand::Generate)
}

#[then]
fn the_command_is_help(world: &TestWorld) -> Result<()> {
    verify_command(world, ExpectedCommand::Help)
}

#[then]
fn the_help_topic_is_targets(world: &TestWorld) -> Result<()> {
    verify_help_topic(world, Some(&HelpTopic::Targets))
}

#[then]
fn the_help_has_no_topic(world: &TestWorld) -> Result<()> {
    verify_help_topic(world, None)
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

#[then("the CLI network policy allows {url:string}")]
fn cli_policy_allows(world: &TestWorld, url: UrlString) -> Result<()> {
    verify_cli_policy_allows(world, &url)
}

#[then("the CLI network policy rejects {url:string} with {message:string}")]
fn cli_policy_rejects(world: &TestWorld, url: UrlString, message: ErrorFragment) -> Result<()> {
    verify_cli_policy_rejects(world, &url, &message)
}

#[then("the generate output path is {path:string}")]
fn generate_output_path(world: &TestWorld, path: PathString) -> Result<()> {
    verify_generate_output_path(world, &path)
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

#[cfg(test)]
mod tests {
    //! Regression coverage for attached BDD directory options.

    use super::insert_discovery_directory_if_missing;
    use rstest::rstest;
    use std::ffi::{OsStr, OsString};

    #[rstest]
    #[case::short("-Cproject")]
    #[case::long("--directory=project")]
    fn attached_directory_options_prevent_default_injection(#[case] attached: &str) {
        let mut tokens = vec![OsString::from("netsuke"), OsString::from(attached)];
        let expected = tokens.clone();

        insert_discovery_directory_if_missing(&mut tokens, OsStr::new("temporary-project"));

        assert_eq!(tokens, expected, "attached option {attached:?}");
    }

    #[cfg(unix)]
    #[rstest]
    #[case::short(b"-C\xff")]
    #[case::long(b"--directory=\xff")]
    fn non_utf8_attached_directory_options_prevent_default_injection(#[case] attached: &[u8]) {
        use std::os::unix::ffi::OsStringExt;

        let mut tokens = vec![
            OsString::from("netsuke"),
            OsString::from_vec(attached.to_vec()),
        ];
        let expected = tokens.clone();

        insert_discovery_directory_if_missing(&mut tokens, OsStr::new("temporary-project"));

        assert_eq!(tokens, expected, "non-UTF-8 attached directory option");
    }
}
