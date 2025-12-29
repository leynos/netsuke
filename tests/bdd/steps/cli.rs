//! Step definitions for CLI parsing scenarios.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::helpers::parse_store::store_parse_outcome;
use crate::bdd::types::{CliArgs, ErrorFragment, JobCount, PathString, TargetName, UrlString};
use anyhow::{Context, Result, anyhow, bail, ensure};
use clap::Parser;
use netsuke::cli::{BuildArgs, Cli, Commands};
use rstest_bdd_macros::{given, then, when};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Apply CLI parsing, storing result or error in world state.
fn apply_cli(world: &TestWorld, args: &CliArgs) {
    let tokens = build_token_list(args);
    let outcome = Cli::try_parse_from(tokens)
        .map(normalize_cli)
        .map_err(|e| e.to_string());
    store_parse_outcome(&world.cli, &world.cli_error, outcome);
}

/// Get the CLI's network policy using `with_ref`.
fn cli_network_policy(world: &TestWorld) -> Result<netsuke::stdlib::NetworkPolicy> {
    world
        .cli
        .with_ref(|cli| cli.network_policy())
        .context("CLI has not been parsed")?
        .context("construct CLI network policy")
}

/// Extract build command args (targets and emit path) using with_ref.
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

/// Get the parsed CLI command using with_ref.
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

/// Build the token list from CLI arguments for parsing.
///
/// Uses shell-like splitting via `shlex` to handle quoted arguments correctly,
/// matching the behaviour of `std::env::args_os()` more closely. Falls back to
/// whitespace splitting if shlex fails (e.g., unbalanced quotes).
fn build_token_list(args: &CliArgs) -> Vec<String> {
    let mut tokens = vec!["netsuke".to_owned()];
    match shlex::split(args.as_str()) {
        Some(mut split_args) => tokens.append(&mut split_args),
        None => tokens.extend(args.as_str().split_whitespace().map(str::to_owned)),
    }
    tokens
}

/// Normalise a parsed CLI by setting default command if missing.
fn normalize_cli(mut cli: Cli) -> Cli {
    if cli.command.is_none() {
        cli.command = Some(Commands::Build(BuildArgs {
            emit: None,
            targets: Vec::new(),
        }));
    }
    cli
}

// ---------------------------------------------------------------------------
// Typed verification helpers
// ---------------------------------------------------------------------------

/// Expected CLI command variants for verification.
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
        reason = "matches! macro is not const-compatible"
    )]
    fn matches(&self, actual: &Commands) -> bool {
        matches!(
            (self, actual),
            (Self::Build, Commands::Build(_))
                | (Self::Clean, Commands::Clean)
                | (Self::Graph, Commands::Graph)
                | (Self::Manifest, Commands::Manifest { .. })
        )
    }

    /// Return the command name for error messages.
    const fn name(&self) -> &'static str {
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
    let result = world.cli.with_ref(|cli| {
        if cli.jobs == Some(expected.value()) {
            Ok(())
        } else {
            Err(anyhow!(
                "expected job count {}, got {:?}",
                expected.value(),
                cli.jobs
            ))
        }
    });
    result.context("CLI has not been parsed")?
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
    let result = world.cli.with_ref(|cli| {
        if cli.file.as_path() == path.as_path() {
            Ok(())
        } else {
            Err(anyhow!(
                "expected manifest path {}, got {}",
                path,
                cli.file.display()
            ))
        }
    });
    result.context("CLI has not been parsed")?
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
    let result = world.cli.with_ref(|cli| {
        if cli.directory.as_deref() == Some(directory.as_path()) {
            Ok(())
        } else {
            Err(anyhow!(
                "expected working directory {}, got {:?}",
                directory,
                cli.directory
            ))
        }
    });
    result.context("CLI has not been parsed")?
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
        other => Err(anyhow!("expected manifest command, got {other:?}")),
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

#[given("the CLI is parsed with {args:string}")]
#[when("the CLI is parsed with {args:string}")]
#[when("the CLI is parsed with invalid arguments {args:string}")]
fn parse_cli(world: &TestWorld, args: &str) -> Result<()> {
    let cli_args = CliArgs::new(args);
    apply_cli(world, &cli_args);
    Ok(())
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("parsing succeeds")]
fn parsing_succeeds(world: &TestWorld) -> Result<()> {
    verify_parsing_succeeded(world)
}

#[then("the command is build")]
fn command_is_build(world: &TestWorld) -> Result<()> {
    verify_command(world, ExpectedCommand::Build)
}

#[then("the command is clean")]
fn command_is_clean(world: &TestWorld) -> Result<()> {
    verify_command(world, ExpectedCommand::Clean)
}

#[then("the command is graph")]
fn command_is_graph(world: &TestWorld) -> Result<()> {
    verify_command(world, ExpectedCommand::Graph)
}

#[then("the command is manifest")]
fn command_is_manifest(world: &TestWorld) -> Result<()> {
    verify_command(world, ExpectedCommand::Manifest)
}

#[then("the manifest path is {path:string}")]
fn manifest_path(world: &TestWorld, path: &str) -> Result<()> {
    verify_manifest_path(world, &PathString::new(path))
}

#[then("the first target is {target:string}")]
fn first_target(world: &TestWorld, target: &str) -> Result<()> {
    verify_first_target(world, &TargetName::new(target))
}

#[then("the working directory is {directory:string}")]
fn working_directory(world: &TestWorld, directory: &str) -> Result<()> {
    verify_working_directory(world, &PathString::new(directory))
}

#[then("the job count is {count:usize}")]
fn job_count(world: &TestWorld, count: usize) -> Result<()> {
    verify_job_count(world, JobCount::new(count))
}

#[then("the emit path is {path:string}")]
fn emit_path(world: &TestWorld, path: &str) -> Result<()> {
    verify_emit_path(world, &PathString::new(path))
}

#[then("the CLI network policy allows {url:string}")]
fn cli_policy_allows(world: &TestWorld, url: &str) -> Result<()> {
    verify_cli_policy_allows(world, &UrlString::new(url))
}

#[then("the CLI network policy rejects {url:string} with {message:string}")]
fn cli_policy_rejects(world: &TestWorld, url: &str, message: &str) -> Result<()> {
    verify_cli_policy_rejects(world, &UrlString::new(url), &ErrorFragment::new(message))
}

#[then("the manifest command path is {path:string}")]
fn manifest_command_path(world: &TestWorld, path: &str) -> Result<()> {
    verify_manifest_command_path(world, &PathString::new(path))
}

#[then("an error should be returned")]
fn error_should_be_returned(world: &TestWorld) -> Result<()> {
    verify_error_returned(world)
}

#[then("the error message should contain {fragment:string}")]
fn error_message_should_contain(world: &TestWorld, fragment: &str) -> Result<()> {
    verify_error_contains(world, &ErrorFragment::new(fragment))
}
