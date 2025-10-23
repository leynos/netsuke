//! Cucumber step definitions for CLI behaviour-driven testing.
//!
//! This module provides step definitions that test the command-line interface
//! parsing and validation using the Cucumber framework.
#![allow(
    clippy::shadow_reuse,
    clippy::shadow_unrelated,
    reason = "Cucumber step macros rebind capture names and steps prefer expect"
)]

use crate::CliWorld;
use anyhow::{Context, Result, anyhow, ensure};
use clap::Parser;
use cucumber::{given, then, when};
use netsuke::cli::{BuildArgs, Cli, Commands};
use std::path::PathBuf;

fn apply_cli(world: &mut CliWorld, args: &str) {
    let tokens: Vec<String> = std::iter::once("netsuke".to_owned())
        .chain(args.split_whitespace().map(str::to_string))
        .collect();
    match Cli::try_parse_from(tokens) {
        Ok(mut cli) => {
            if cli.command.is_none() {
                cli.command = Some(Commands::Build(BuildArgs {
                    emit: None,
                    targets: Vec::new(),
                }));
            }
            world.cli = Some(cli);
            world.cli_error = None;
        }
        Err(e) => {
            world.cli = None;
            world.cli_error = Some(e.to_string());
        }
    }
}

fn extract_build(world: &CliWorld) -> Result<(&Vec<String>, &Option<PathBuf>)> {
    let cli = world.cli.as_ref().context("CLI has not been parsed")?;
    let command = cli.command.as_ref().context("CLI command missing")?;
    match command {
        Commands::Build(args) => Ok((&args.targets, &args.emit)),
        other => Err(anyhow!("expected build command, got {other:?}")),
    }
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[given(expr = "the CLI is parsed with {string}")]
#[when(expr = "the CLI is parsed with {string}")]
fn parse_cli(world: &mut CliWorld, cli_args: String) {
    apply_cli(world, &cli_args);
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[given(expr = "the CLI is parsed with invalid arguments {string}")]
#[when(expr = "the CLI is parsed with invalid arguments {string}")]
fn parse_cli_invalid(world: &mut CliWorld, invalid_args: String) {
    apply_cli(world, &invalid_args);
}

#[then("parsing succeeds")]
fn parsing_succeeds(world: &mut CliWorld) -> Result<()> {
    ensure!(world.cli.is_some(), "CLI should be present after parsing");
    Ok(())
}

#[then("the command is build")]
fn command_is_build(world: &mut CliWorld) -> Result<()> {
    let _ = extract_build(world)?;
    Ok(())
}

#[then("the command is clean")]
fn command_is_clean(world: &mut CliWorld) -> Result<()> {
    let cli = world.cli.as_ref().context("CLI has not been parsed")?;
    let command = cli.command.as_ref().context("CLI command missing")?;
    ensure!(
        matches!(command, Commands::Clean),
        "command should be clean"
    );
    Ok(())
}

#[then("the command is graph")]
fn command_is_graph(world: &mut CliWorld) -> Result<()> {
    let cli = world.cli.as_ref().context("CLI has not been parsed")?;
    let command = cli.command.as_ref().context("CLI command missing")?;
    ensure!(
        matches!(command, Commands::Graph),
        "command should be graph"
    );
    Ok(())
}

#[then("the command is manifest")]
fn command_is_manifest(world: &mut CliWorld) -> Result<()> {
    let cli = world.cli.as_ref().context("CLI has not been parsed")?;
    let command = cli.command.as_ref().context("CLI command missing")?;
    ensure!(
        matches!(command, Commands::Manifest { .. }),
        "command should be manifest"
    );
    Ok(())
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the manifest path is {string}")]
fn manifest_path(world: &mut CliWorld, manifest_path_str: String) -> Result<()> {
    let cli = world.cli.as_ref().context("CLI has not been parsed")?;
    ensure!(
        cli.file == PathBuf::from(&manifest_path_str),
        "expected manifest path {}, got {}",
        manifest_path_str,
        cli.file.display()
    );
    Ok(())
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the first target is {string}")]
fn first_target(world: &mut CliWorld, expected_target: String) -> Result<()> {
    let (targets, _) = extract_build(world)?;
    ensure!(
        targets.first() == Some(&expected_target),
        "expected first target {}, got {:?}",
        expected_target,
        targets.first()
    );
    Ok(())
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the working directory is {string}")]
fn working_directory(world: &mut CliWorld, directory: String) -> Result<()> {
    let cli = world.cli.as_ref().context("CLI has not been parsed")?;
    ensure!(
        cli.directory.as_ref() == Some(&PathBuf::from(&directory)),
        "expected working directory {}, got {:?}",
        directory,
        cli.directory
    );
    Ok(())
}

#[then(expr = "the job count is {int}")]
fn job_count(world: &mut CliWorld, expected_jobs: usize) -> Result<()> {
    let cli = world.cli.as_ref().context("CLI has not been parsed")?;
    ensure!(
        cli.jobs == Some(expected_jobs),
        "expected job count {}, got {:?}",
        expected_jobs,
        cli.jobs
    );
    Ok(())
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the emit path is {string}")]
fn emit_path(world: &mut CliWorld, emit_path_str: String) -> Result<()> {
    let (_, emit) = extract_build(world)?;
    ensure!(
        emit.as_ref() == Some(&PathBuf::from(&emit_path_str)),
        "expected emit path {}, got {:?}",
        emit_path_str,
        emit
    );
    Ok(())
}
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the manifest command path is {string}")]
fn manifest_command_path(world: &mut CliWorld, manifest_output_path: String) -> Result<()> {
    let cli = world.cli.as_ref().context("CLI has not been parsed")?;
    let command = cli.command.as_ref().context("CLI command missing")?;
    match command {
        Commands::Manifest { file } => {
            ensure!(
                file == &PathBuf::from(&manifest_output_path),
                "expected manifest output {}, got {}",
                manifest_output_path,
                file.display()
            );
            Ok(())
        }
        other => Err(anyhow!("expected manifest command, got {other:?}")),
    }
}

#[then("an error should be returned")]
fn error_should_be_returned(world: &mut CliWorld) -> Result<()> {
    ensure!(
        world.cli_error.is_some(),
        "Expected an error, but none was returned"
    );
    Ok(())
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the error message should contain {string}")]
fn error_message_should_contain(world: &mut CliWorld, expected_fragment: String) -> Result<()> {
    let error = world
        .cli_error
        .as_ref()
        .context("no error was returned by CLI parsing")?;
    ensure!(
        error.contains(&expected_fragment),
        "Error message '{error}' does not contain expected '{expected_fragment}'"
    );
    Ok(())
}
