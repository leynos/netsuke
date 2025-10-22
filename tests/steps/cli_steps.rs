//! Cucumber step definitions for CLI behaviour-driven testing.
//!
//! This module provides step definitions that test the command-line interface
//! parsing and validation using the Cucumber framework.
#![allow(
    clippy::shadow_reuse,
    clippy::shadow_unrelated,
    clippy::expect_used,
    reason = "Cucumber step macros rebind capture names and steps prefer expect"
)]

use crate::CliWorld;
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

fn extract_build(world: &CliWorld) -> Option<(&Vec<String>, &Option<PathBuf>)> {
    let cli = world.cli.as_ref()?;
    match cli.command.as_ref()? {
        Commands::Build(args) => Some((&args.targets, &args.emit)),
        _ => None,
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
fn parsing_succeeds(world: &mut CliWorld) {
    assert!(world.cli.is_some());
}

#[then("the command is build")]
fn command_is_build(world: &mut CliWorld) {
    assert!(extract_build(world).is_some(), "command should be build");
}

#[then("the command is clean")]
fn command_is_clean(world: &mut CliWorld) {
    let cli = world.cli.as_ref().expect("cli");
    assert!(matches!(
        cli.command.as_ref().expect("command"),
        Commands::Clean
    ));
}

#[then("the command is graph")]
fn command_is_graph(world: &mut CliWorld) {
    let cli = world.cli.as_ref().expect("cli");
    assert!(matches!(
        cli.command.as_ref().expect("command"),
        Commands::Graph
    ));
}

#[then("the command is manifest")]
fn command_is_manifest(world: &mut CliWorld) {
    let cli = world.cli.as_ref().expect("cli");
    assert!(matches!(
        cli.command.as_ref().expect("command"),
        Commands::Manifest { .. }
    ));
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the manifest path is {string}")]
fn manifest_path(world: &mut CliWorld, manifest_path_str: String) {
    let cli = world.cli.as_ref().expect("cli");
    assert_eq!(cli.file, PathBuf::from(&manifest_path_str));
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the first target is {string}")]
fn first_target(world: &mut CliWorld, expected_target: String) {
    let (targets, _) = extract_build(world).expect("command should be build");
    assert_eq!(targets.first(), Some(&expected_target));
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the working directory is {string}")]
fn working_directory(world: &mut CliWorld, directory: String) {
    let cli = world.cli.as_ref().expect("cli");
    assert_eq!(cli.directory.as_ref(), Some(&PathBuf::from(&directory)));
}

#[then(expr = "the job count is {int}")]
fn job_count(world: &mut CliWorld, expected_jobs: usize) {
    let cli = world.cli.as_ref().expect("cli");
    assert_eq!(cli.jobs, Some(expected_jobs));
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the emit path is {string}")]
fn emit_path(world: &mut CliWorld, emit_path_str: String) {
    let (_, emit) = extract_build(world).expect("command should be build");
    assert_eq!(emit.as_ref(), Some(&PathBuf::from(&emit_path_str)));
}
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the manifest command path is {string}")]
fn manifest_command_path(world: &mut CliWorld, manifest_output_path: String) {
    let cli = world.cli.as_ref().expect("cli");
    match cli.command.as_ref().expect("command") {
        Commands::Manifest { file } => {
            assert_eq!(file, &PathBuf::from(&manifest_output_path));
        }
        _ => panic!("command should be manifest"),
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
fn error_message_should_contain(world: &mut CliWorld, expected_fragment: String) {
    let error = world.cli_error.as_ref().expect("No error was returned");
    assert!(
        error.contains(&expected_fragment),
        "Error message '{error}' does not contain expected '{expected_fragment}'"
    );
}
