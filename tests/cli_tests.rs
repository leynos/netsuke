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

struct CliCase {
    argv: Vec<&'static str>,
    file: PathBuf,
    directory: Option<PathBuf>,
    jobs: Option<usize>,
    verbose: bool,
    allow_scheme: Vec<String>,
    allow_host: Vec<String>,
    block_host: Vec<String>,
    default_deny: bool,
    expected_cmd: Commands,
}

#[rstest]
#[case(CliCase {
    argv: vec!["netsuke"],
    file: PathBuf::from("Netsukefile"),
    directory: None,
    jobs: None,
    verbose: false,
    allow_scheme: Vec::new(),
    allow_host: Vec::new(),
    block_host: Vec::new(),
    default_deny: false,
    expected_cmd: Commands::Build(BuildArgs { emit: None, targets: Vec::new() }),
})]
#[case(CliCase {
    argv: vec!["netsuke", "--file", "alt.yml", "-C", "work", "-j", "4", "build", "a", "b"],
    file: PathBuf::from("alt.yml"),
    directory: Some(PathBuf::from("work")),
    jobs: Some(4),
    verbose: false,
    allow_scheme: Vec::new(),
    allow_host: Vec::new(),
    block_host: Vec::new(),
    default_deny: false,
    expected_cmd: Commands::Build(BuildArgs { emit: None, targets: vec!["a".into(), "b".into()] }),
})]
#[case(CliCase {
    argv: vec!["netsuke", "--verbose"],
    file: PathBuf::from("Netsukefile"),
    directory: None,
    jobs: None,
    verbose: true,
    allow_scheme: Vec::new(),
    allow_host: Vec::new(),
    block_host: Vec::new(),
    default_deny: false,
    expected_cmd: Commands::Build(BuildArgs { emit: None, targets: Vec::new() }),
})]
#[case(CliCase {
    argv: vec!["netsuke", "build", "--emit", "out.ninja", "a"],
    file: PathBuf::from("Netsukefile"),
    directory: None,
    jobs: None,
    verbose: false,
    allow_scheme: Vec::new(),
    allow_host: Vec::new(),
    block_host: Vec::new(),
    default_deny: false,
    expected_cmd: Commands::Build(BuildArgs { emit: Some(PathBuf::from("out.ninja")), targets: vec!["a".into()] }),
})]
#[case(CliCase {
    argv: vec!["netsuke", "manifest", "out.ninja"],
    file: PathBuf::from("Netsukefile"),
    directory: None,
    jobs: None,
    verbose: false,
    allow_scheme: Vec::new(),
    allow_host: Vec::new(),
    block_host: Vec::new(),
    default_deny: false,
    expected_cmd: Commands::Manifest { file: PathBuf::from("out.ninja") },
})]
#[case(CliCase {
    argv: vec![
        "netsuke",
        "--fetch-allow-scheme",
        "http",
        "--fetch-default-deny",
        "--fetch-allow-host",
        "example.com",
        "--fetch-block-host",
        "deny.test",
    ],
    file: PathBuf::from("Netsukefile"),
    directory: None,
    jobs: None,
    verbose: false,
    allow_scheme: vec![String::from("http")],
    allow_host: vec![String::from("example.com")],
    block_host: vec![String::from("deny.test")],
    default_deny: true,
    expected_cmd: Commands::Build(BuildArgs { emit: None, targets: Vec::new() }),
})]
fn parse_cli(#[case] case: CliCase) -> Result<()> {
    let cli = Cli::parse_from_with_default(case.argv.clone());
    ensure!(cli.file == case.file, "parsed file should match input");
    ensure!(
        cli.directory == case.directory,
        "parsed directory should match input"
    );
    ensure!(cli.jobs == case.jobs, "parsed jobs should match input");
    ensure!(
        cli.verbose == case.verbose,
        "verbose flag should match input"
    );
    ensure!(
        cli.fetch_allow_scheme == case.allow_scheme,
        "allow-scheme flags should match input"
    );
    ensure!(
        cli.fetch_allow_host == case.allow_host,
        "allow-host flags should match input"
    );
    ensure!(
        cli.fetch_block_host == case.block_host,
        "block-host flags should match input"
    );
    ensure!(
        cli.fetch_default_deny == case.default_deny,
        "default-deny flag should match input"
    );
    let command = cli.command.context("command should be set")?;
    ensure!(
        command == case.expected_cmd,
        "parsed command should match expected {:?}",
        case.expected_cmd
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
