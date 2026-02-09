//! Step definitions for CLI parsing scenarios.
//!
//! Provides BDD step functions for parsing command-line arguments via `clap`,
//! verifying parsed commands, and checking CLI network policy behaviour.
//! Steps store results in [`TestWorld`] for downstream assertions.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::helpers::parse_store::store_parse_outcome;
use crate::bdd::helpers::tokens::build_tokens;
use crate::bdd::types::{CliArgs, ErrorFragment, JobCount, PathString, TargetName, UrlString};
use anyhow::{Context, Result, bail, ensure};
use netsuke::cli::{Cli, Commands};
use netsuke::cli_localization;
use netsuke::locale_resolution;
use rstest_bdd_macros::{given, then, when};
use std::path::PathBuf;
use std::sync::Arc;
use test_support::locale_stubs::{StubEnv, StubSystemLocale};

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Apply CLI parsing, storing result or error in world state.
fn apply_cli(world: &TestWorld, args: &CliArgs) {
    let env = StubEnv {
        locale: world.locale_env.get(),
    };
    let system = StubSystemLocale {
        locale: world.locale_system.get(),
    };
    let tokens = build_tokens(args.as_str());
    let locale = locale_resolution::resolve_startup_locale(&tokens, &env, &system);
    let localizer = Arc::from(cli_localization::build_localizer(locale.as_deref()));
    let outcome = netsuke::cli::parse_with_localizer_from(tokens, &localizer)
        .map(|(cli, _matches)| normalize_cli(cli))
        .map_err(|e| e.to_string());
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

// ---------------------------------------------------------------------------
// Typed verification helpers
// ---------------------------------------------------------------------------

/// Expected CLI command variants for verification.
#[derive(Copy, Clone)]
enum ExpectedCommand {
    Build,
    Clean,
    Graph,
    Manifest,
}

impl ExpectedCommand {
    /// Check if the actual command matches the expected variant.
    #[expect(
        clippy::missing_const_for_fn,
        reason = "Commands contains heap-allocated types preventing const evaluation"
    )]
    fn matches(self, actual: &Commands) -> bool {
        matches!(
            (self, actual),
            (Self::Build, Commands::Build(_))
                | (Self::Clean, Commands::Clean)
                | (Self::Graph, Commands::Graph)
                | (Self::Manifest, Commands::Manifest { .. })
        )
    }

    /// Return the command name for error messages.
    const fn name(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Clean => "clean",
            Self::Graph => "graph",
            Self::Manifest => "manifest",
        }
    }
}

fn verify_command(world: &TestWorld, expected: ExpectedCommand) -> Result<()> {
    let command = get_command(world)?;
    ensure!(
        expected.matches(&command),
        "command should be {}",
        expected.name()
    );
    Ok(())
}

fn verify_job_count(world: &TestWorld, expected: JobCount) -> Result<()> {
    let actual = world
        .cli
        .with_ref(|cli| cli.jobs)
        .context("CLI has not been parsed")?;
    ensure!(
        actual == Some(expected.value()),
        "expected job count {}, got {:?}",
        expected.value(),
        actual
    );
    Ok(())
}

fn verify_parsing_succeeded(world: &TestWorld) -> Result<()> {
    ensure!(world.cli.is_some(), "CLI should be present after parsing");
    Ok(())
}

fn verify_error_returned(world: &TestWorld) -> Result<()> {
    ensure!(
        world.cli_error.is_filled(),
        "Expected an error, but none was returned"
    );
    Ok(())
}

fn verify_manifest_path(world: &TestWorld, path: &PathString) -> Result<()> {
    let actual = world
        .cli
        .with_ref(|cli| cli.file.clone())
        .context("CLI has not been parsed")?;
    ensure!(
        actual.as_path() == path.as_path(),
        "expected manifest path {}, got {}",
        path,
        actual.display()
    );
    Ok(())
}

fn verify_first_target(world: &TestWorld, target: &TargetName) -> Result<()> {
    let (targets, _) = extract_build(world)?;
    ensure!(
        targets.first().map(String::as_str) == Some(target.as_str()),
        "expected first target {}, got {:?}",
        target,
        targets.first()
    );
    Ok(())
}

fn verify_working_directory(world: &TestWorld, directory: &PathString) -> Result<()> {
    let actual = world
        .cli
        .with_ref(|cli| cli.directory.clone())
        .context("CLI has not been parsed")?;
    ensure!(
        actual.as_deref() == Some(directory.as_path()),
        "expected working directory {}, got {:?}",
        directory,
        actual
    );
    Ok(())
}

fn verify_emit_path(world: &TestWorld, path: &PathString) -> Result<()> {
    let (_, emit) = extract_build(world)?;
    ensure!(
        emit.as_deref() == Some(path.as_path()),
        "expected emit path {}, got {:?}",
        path,
        emit
    );
    Ok(())
}

fn verify_cli_policy_allows(world: &TestWorld, url: &UrlString) -> Result<()> {
    let policy = cli_network_policy(world)?;
    let parsed = url.parse().context("parse URL for CLI policy check")?;
    ensure!(
        policy.evaluate(&parsed).is_ok(),
        "expected CLI policy to allow {}",
        url,
    );
    Ok(())
}

fn verify_cli_policy_rejects(
    world: &TestWorld,
    url: &UrlString,
    message: &ErrorFragment,
) -> Result<()> {
    let policy = cli_network_policy(world)?;
    let parsed = url.parse().context("parse URL for CLI policy check")?;
    let Err(err) = policy.evaluate(&parsed) else {
        bail!("expected CLI policy to reject {}", url);
    };
    ensure!(
        err.to_string().contains(message.as_str()),
        "expected error to mention '{}', got '{err}'",
        message,
    );
    Ok(())
}

fn verify_manifest_command_path(world: &TestWorld, path: &PathString) -> Result<()> {
    let command = get_command(world)?;
    match command {
        Commands::Manifest { file } => {
            ensure!(
                file == path.to_path_buf(),
                "expected manifest output {}, got {}",
                path,
                file.display()
            );
            Ok(())
        }
        other => bail!("expected manifest command, got {other:?}"),
    }
}

fn verify_error_contains(world: &TestWorld, fragment: &ErrorFragment) -> Result<()> {
    let error = world
        .cli_error
        .get()
        .context("no error was returned by CLI parsing")?;
    ensure!(
        error.contains(fragment.as_str()),
        "Error message '{error}' does not contain expected '{}'",
        fragment
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Given/When steps
// ---------------------------------------------------------------------------

#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[given("the CLI is parsed with {args:string}")]
#[when("the CLI is parsed with {args:string}")]
#[when("the CLI is parsed with invalid arguments {args:string}")]
fn parse_cli(world: &TestWorld, args: CliArgs) -> Result<()> {
    apply_cli(world, &args);
    Ok(())
}

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

#[then]
fn an_error_should_be_returned(world: &TestWorld) -> Result<()> {
    verify_error_returned(world)
}

#[then("the error message should contain {fragment:string}")]
fn error_message_should_contain(world: &TestWorld, fragment: ErrorFragment) -> Result<()> {
    verify_error_contains(world, &fragment)
}
