//! Cucumber step definitions for CLI behaviour-driven testing.
//!
//! This module provides step definitions that test the command-line interface
//! parsing and validation using the Cucumber framework.

use crate::CliWorld;
use clap::Parser;
use cucumber::{then, when};
use netsuke::cli::{Cli, Commands};
use std::path::PathBuf;

fn apply_cli(world: &mut CliWorld, args: &str) {
    let tokens: Vec<String> = std::iter::once("netsuke".to_string())
        .chain(args.split_whitespace().map(str::to_string))
        .collect();
    match Cli::try_parse_from(tokens) {
        Ok(mut cli) => {
            if cli.command.is_none() {
                cli.command = Some(Commands::Build {
                    targets: Vec::new(),
                });
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

fn extract_build(world: &CliWorld) -> &Vec<String> {
    let cli = world.cli.as_ref().expect("cli");
    match cli.command.as_ref().expect("command") {
        Commands::Build { targets } => targets,
        other => panic!("Expected build command, got {other:?}"),
    }
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[when(expr = "the CLI is parsed with {string}")]
fn parse_cli(world: &mut CliWorld, args: String) {
    apply_cli(world, &args);
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[when(expr = "the CLI is parsed with invalid arguments {string}")]
fn parse_cli_invalid(world: &mut CliWorld, args: String) {
    apply_cli(world, &args);
}

#[then("parsing succeeds")]
fn parsing_succeeds(world: &mut CliWorld) {
    assert!(world.cli.is_some());
}

#[then("the command is build")]
fn command_is_build(world: &mut CliWorld) {
    let _ = extract_build(world);
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the manifest path is {string}")]
fn manifest_path(world: &mut CliWorld, path: String) {
    let cli = world.cli.as_ref().expect("cli");
    assert_eq!(cli.file, PathBuf::from(path));
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the first target is {string}")]
fn first_target(world: &mut CliWorld, target: String) {
    assert_eq!(extract_build(world).first(), Some(&target));
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the working directory is {string}")]
fn working_directory(world: &mut CliWorld, dir: String) {
    let cli = world.cli.as_ref().expect("cli");
    assert_eq!(cli.directory.as_ref(), Some(&PathBuf::from(dir)));
}

#[then(expr = "the job count is {int}")]
fn job_count(world: &mut CliWorld, jobs: usize) {
    let cli = world.cli.as_ref().expect("cli");
    assert_eq!(cli.jobs, Some(jobs));
}

#[then("an error should be returned")]
fn error_should_be_returned(world: &mut CliWorld) {
    assert!(
        world.cli_error.is_some(),
        "Expected an error, but none was returned"
    );
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the error message should contain {string}")]
fn error_message_should_contain(world: &mut CliWorld, expected: String) {
    let error = world.cli_error.as_ref().expect("No error was returned");
    assert!(
        error.contains(&expected),
        "Error message '{error}' does not contain expected '{expected}'"
    );
}
