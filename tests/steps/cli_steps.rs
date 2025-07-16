//! Cucumber step definitions for CLI behaviour-driven testing.
//!
//! This module provides step definitions that test the command-line interface
//! parsing and validation using the Cucumber framework.

use crate::CliWorld;
use clap::Parser;
use cucumber::{then, when};
use netsuke::cli::{Cli, Commands};
use std::path::PathBuf;

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[when(expr = "the CLI is parsed with {string}")]
fn parse_cli(world: &mut CliWorld, args: String) {
    let tokens: Vec<String> = if args.is_empty() {
        vec!["netsuke".to_string()]
    } else {
        std::iter::once("netsuke".to_string())
            .chain(args.split_whitespace().map(str::to_string))
            .collect()
    };
    match Cli::try_parse_from(tokens) {
        Ok(cli) => {
            world.cli = Some(cli);
            world.cli_error = None;
        }
        Err(e) => {
            world.cli = None;
            world.cli_error = Some(e.to_string());
        }
    }
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[when(expr = "the CLI is parsed with invalid arguments {string}")]
fn parse_cli_invalid(world: &mut CliWorld, args: String) {
    let tokens: Vec<String> = std::iter::once("netsuke".to_string())
        .chain(args.split_whitespace().map(str::to_string))
        .collect();
    match Cli::try_parse_from(tokens) {
        Ok(cli) => {
            world.cli = Some(cli);
            world.cli_error = None;
        }
        Err(e) => {
            world.cli = None;
            world.cli_error = Some(e.to_string());
        }
    }
}

#[then("parsing succeeds")]
fn parsing_succeeds(world: &mut CliWorld) {
    assert!(world.cli.is_some());
}

#[then("the command is build")]
fn command_is_build(world: &mut CliWorld) {
    let cli = world.cli.as_ref().expect("cli");
    match cli
        .command
        .as_ref()
        .unwrap_or(&Commands::Build { targets: vec![] })
    {
        Commands::Build { .. } => (),
        other => panic!("Expected build command, got {other:?}"),
    }
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
    let cli = world.cli.as_ref().expect("cli");
    match cli
        .command
        .as_ref()
        .unwrap_or(&Commands::Build { targets: vec![] })
    {
        Commands::Build { targets } => assert_eq!(targets.first(), Some(&target)),
        other => panic!("Expected build command, got {other:?}"),
    }
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
