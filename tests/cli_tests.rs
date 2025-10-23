//! Unit tests for CLI argument parsing and validation.
//!
//! This module exercises the command-line interface defined in [`netsuke::cli`]
//! using `rstest` for parameterised coverage of success and error scenarios.

use anyhow::{Context, Result, ensure};
use clap::Parser;
use clap::error::ErrorKind;
use netsuke::cli::{BuildArgs, Cli, Commands};
use rstest::rstest;
use std::path::PathBuf;

#[rstest]
#[case(vec!["netsuke"], PathBuf::from("Netsukefile"), None, None, false, Commands::Build(BuildArgs { emit: None, targets: Vec::new() }))]
#[case(
    vec!["netsuke", "--file", "alt.yml", "-C", "work", "-j", "4", "build", "a", "b"],
    PathBuf::from("alt.yml"),
    Some(PathBuf::from("work")),
    Some(4),
    false,
    Commands::Build(BuildArgs { emit: None, targets: vec!["a".into(), "b".into()] }),
)]
#[case(vec!["netsuke", "--verbose"], PathBuf::from("Netsukefile"), None, None, true, Commands::Build(BuildArgs { emit: None, targets: Vec::new() }))]
#[case(
    vec!["netsuke", "build", "--emit", "out.ninja", "a"],
    PathBuf::from("Netsukefile"),
    None,
    None,
    false,
    Commands::Build(BuildArgs { emit: Some(PathBuf::from("out.ninja")), targets: vec!["a".into()] }),
)]
#[case(
    vec!["netsuke", "manifest", "out.ninja"],
    PathBuf::from("Netsukefile"),
    None,
    None,
    false,
    Commands::Manifest { file: PathBuf::from("out.ninja") },
)]
fn parse_cli(
    #[case] argv: Vec<&str>,
    #[case] file: PathBuf,
    #[case] directory: Option<PathBuf>,
    #[case] jobs: Option<usize>,
    #[case] verbose: bool,
    #[case] expected_cmd: Commands,
) -> Result<()> {
    let cli = Cli::parse_from_with_default(argv.clone());
    ensure!(cli.file == file, "parsed file should match input");
    ensure!(
        cli.directory == directory,
        "parsed directory should match input"
    );
    ensure!(cli.jobs == jobs, "parsed jobs should match input");
    ensure!(cli.verbose == verbose, "verbose flag should match input");
    let command = cli.command.context("command should be set")?;
    ensure!(
        command == expected_cmd,
        "parsed command should match expected {:?}",
        expected_cmd
    );
    Ok(())
}

#[rstest]
#[case(vec!["netsuke", "unknowncmd"], ErrorKind::InvalidSubcommand)]
#[case(vec!["netsuke", "--file"], ErrorKind::InvalidValue)]
#[case(vec!["netsuke", "-j", "notanumber"], ErrorKind::ValueValidation)]
#[case(vec!["netsuke", "--file", "alt.yml", "-C"], ErrorKind::InvalidValue)]
#[case(vec!["netsuke", "manifest"], ErrorKind::MissingRequiredArgument)]
fn parse_cli_errors(#[case] argv: Vec<&str>, #[case] expected_error: ErrorKind) -> Result<()> {
    let err = Cli::try_parse_from(argv)
        .err()
        .context("parser should reject invalid arguments")?;
    ensure!(
        err.kind() == expected_error,
        "expected error kind {:?}, got {:?}",
        expected_error,
        err.kind()
    );
    Ok(())
}
