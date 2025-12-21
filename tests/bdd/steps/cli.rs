//! Step definitions for CLI parsing scenarios.

use crate::bdd::fixtures::{RefCellOptionExt, with_world};
use crate::bdd::types::{CliArgs, ErrorFragment, PathString, TargetName, UrlString};
use anyhow::{Context, Result, anyhow, bail, ensure};
use clap::Parser;
use netsuke::cli::{BuildArgs, Cli, Commands};
use rstest_bdd_macros::{given, then, when};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Apply CLI parsing, storing result or error in world state.
fn apply_cli(args: &CliArgs) {
    with_world(|world| {
        let tokens: Vec<String> = std::iter::once("netsuke".to_owned())
            .chain(args.as_str().split_whitespace().map(str::to_string))
            .collect();
        match Cli::try_parse_from(tokens) {
            Ok(mut cli) => {
                if cli.command.is_none() {
                    cli.command = Some(Commands::Build(BuildArgs {
                        emit: None,
                        targets: Vec::new(),
                    }));
                }
                world.cli.set_value(cli);
                world.cli_error.clear();
            }
            Err(e) => {
                world.cli.clear_value();
                world.cli_error.set(e.to_string());
            }
        }
    });
}

/// Get the CLI's network policy using `with_ref`.
fn cli_network_policy() -> Result<netsuke::stdlib::NetworkPolicy> {
    with_world(|world| {
        world
            .cli
            .with_ref(|cli| cli.network_policy())
            .context("CLI has not been parsed")?
            .context("construct CLI network policy")
    })
}

/// Extract build command args (targets and emit path) using with_ref.
fn extract_build() -> Result<(Vec<String>, Option<PathBuf>)> {
    with_world(|world| {
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
    })
}

/// Get the parsed CLI command using with_ref.
fn get_command() -> Result<Commands> {
    with_world(|world| {
        world
            .cli
            .with_ref(|cli| cli.command.clone())
            .context("CLI has not been parsed")?
            .context("CLI command missing")
    })
}

// ---------------------------------------------------------------------------
// Typed verification helpers
// ---------------------------------------------------------------------------

fn verify_manifest_path(path: &PathString) -> Result<()> {
    with_world(|world| {
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
    })
}

fn verify_first_target(target: &TargetName) -> Result<()> {
    let (targets, _) = extract_build()?;
    ensure!(
        targets.first().map(String::as_str) == Some(target.as_str()),
        "expected first target {}, got {:?}",
        target,
        targets.first()
    );
    Ok(())
}

fn verify_working_directory(directory: &PathString) -> Result<()> {
    with_world(|world| {
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
    })
}

fn verify_emit_path(path: &PathString) -> Result<()> {
    let (_, emit) = extract_build()?;
    ensure!(
        emit.as_deref() == Some(path.as_path()),
        "expected emit path {}, got {:?}",
        path,
        emit
    );
    Ok(())
}

fn verify_cli_policy_allows(url: &UrlString) -> Result<()> {
    let policy = cli_network_policy()?;
    let parsed = url.parse().context("parse URL for CLI policy check")?;
    ensure!(
        policy.evaluate(&parsed).is_ok(),
        "expected CLI policy to allow {}",
        url,
    );
    Ok(())
}

fn verify_cli_policy_rejects(url: &UrlString, message: &ErrorFragment) -> Result<()> {
    let policy = cli_network_policy()?;
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

fn verify_manifest_command_path(path: &PathString) -> Result<()> {
    let command = get_command()?;
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

fn verify_error_contains(fragment: &ErrorFragment) -> Result<()> {
    with_world(|world| {
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
    })
}

// ---------------------------------------------------------------------------
// Given/When steps
// ---------------------------------------------------------------------------

#[given("the CLI is parsed with {args}")]
fn parse_cli_given(args: String) -> Result<()> {
    apply_cli(&CliArgs::new(args));
    Ok(())
}

#[when("the CLI is parsed with {args}")]
fn parse_cli_when(args: String) -> Result<()> {
    apply_cli(&CliArgs::new(args));
    Ok(())
}

#[given("the CLI is parsed with invalid arguments {args}")]
fn parse_cli_invalid_given(args: String) -> Result<()> {
    apply_cli(&CliArgs::new(args));
    Ok(())
}

#[when("the CLI is parsed with invalid arguments {args}")]
fn parse_cli_invalid_when(args: String) -> Result<()> {
    apply_cli(&CliArgs::new(args));
    Ok(())
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("parsing succeeds")]
fn parsing_succeeds() -> Result<()> {
    with_world(|world| {
        ensure!(world.cli.is_some(), "CLI should be present after parsing");
        Ok(())
    })
}

#[then("the command is build")]
fn command_is_build() -> Result<()> {
    let _ = extract_build()?;
    Ok(())
}

#[then("the command is clean")]
fn command_is_clean() -> Result<()> {
    let command = get_command()?;
    ensure!(
        matches!(command, Commands::Clean),
        "command should be clean"
    );
    Ok(())
}

#[then("the command is graph")]
fn command_is_graph() -> Result<()> {
    let command = get_command()?;
    ensure!(
        matches!(command, Commands::Graph),
        "command should be graph"
    );
    Ok(())
}

#[then("the command is manifest")]
fn command_is_manifest() -> Result<()> {
    let command = get_command()?;
    ensure!(
        matches!(command, Commands::Manifest { .. }),
        "command should be manifest"
    );
    Ok(())
}

#[then("the manifest path is {path}")]
fn manifest_path(path: String) -> Result<()> {
    verify_manifest_path(&PathString::new(path))
}

#[then("the first target is {target}")]
fn first_target(target: String) -> Result<()> {
    verify_first_target(&TargetName::new(target))
}

#[then("the working directory is {directory}")]
fn working_directory(directory: String) -> Result<()> {
    verify_working_directory(&PathString::new(directory))
}

#[then("the job count is {count:usize}")]
fn job_count(count: usize) -> Result<()> {
    with_world(|world| {
        let result = world.cli.with_ref(|cli| {
            if cli.jobs == Some(count) {
                Ok(())
            } else {
                Err(anyhow!("expected job count {}, got {:?}", count, cli.jobs))
            }
        });
        result.context("CLI has not been parsed")?
    })
}

#[then("the emit path is {path}")]
fn emit_path(path: String) -> Result<()> {
    verify_emit_path(&PathString::new(path))
}

#[then("the CLI network policy allows {url}")]
fn cli_policy_allows(url: String) -> Result<()> {
    verify_cli_policy_allows(&UrlString::new(url))
}

#[then("the CLI network policy rejects {url} with {message}")]
fn cli_policy_rejects(url: String, message: String) -> Result<()> {
    verify_cli_policy_rejects(&UrlString::new(url), &ErrorFragment::new(message))
}

#[then("the manifest command path is {path}")]
fn manifest_command_path(path: String) -> Result<()> {
    verify_manifest_command_path(&PathString::new(path))
}

#[then("an error should be returned")]
fn error_should_be_returned() -> Result<()> {
    with_world(|world| {
        ensure!(
            world.cli_error.is_filled(),
            "Expected an error, but none was returned"
        );
        Ok(())
    })
}

#[then("the error message should contain {fragment}")]
fn error_message_should_contain(fragment: String) -> Result<()> {
    verify_error_contains(&ErrorFragment::new(fragment))
}
