//! Unit tests for CLI argument parsing and validation.
//!
//! This module exercises the command-line interface defined in [`netsuke::cli`]
//! using `rstest` for parameterised coverage of success and error scenarios.

use anyhow::{Context, Result, bail, ensure};
use clap::Parser;
use clap::error::ErrorKind;
use netsuke::cli::{BuildArgs, Cli, Commands};
use netsuke::host_pattern::HostPattern;
use netsuke::stdlib::NetworkPolicyViolation;
use rstest::rstest;
use std::path::PathBuf;
use url::Url;

struct CliCase {
    argv: Vec<&'static str>,
    file: PathBuf,
    directory: Option<PathBuf>,
    jobs: Option<usize>,
    verbose: bool,
    allow_scheme: Vec<String>,
    allow_host: Vec<&'static str>,
    block_host: Vec<&'static str>,
    default_deny: bool,
    expected_cmd: Commands,
}

impl Default for CliCase {
    fn default() -> Self {
        Self {
            argv: Vec::new(),
            file: PathBuf::from("Netsukefile"),
            directory: None,
            jobs: None,
            verbose: false,
            allow_scheme: Vec::new(),
            allow_host: Vec::new(),
            block_host: Vec::new(),
            default_deny: false,
            expected_cmd: Commands::Build(BuildArgs {
                emit: None,
                targets: Vec::new(),
            }),
        }
    }
}

#[rstest]
#[case(CliCase { argv: vec!["netsuke"], ..CliCase::default() })]
#[case(CliCase {
    argv: vec!["netsuke", "--file", "alt.yml", "-C", "work", "-j", "4", "build", "a", "b"],
    file: PathBuf::from("alt.yml"),
    directory: Some(PathBuf::from("work")),
    jobs: Some(4),
    expected_cmd: Commands::Build(BuildArgs {
        emit: None,
        targets: vec!["a".into(), "b".into()],
    }),
    ..CliCase::default()
})]
#[case(CliCase {
    argv: vec!["netsuke", "--verbose"],
    verbose: true,
    ..CliCase::default()
})]
#[case(CliCase {
    argv: vec!["netsuke", "build", "--emit", "out.ninja", "a"],
    expected_cmd: Commands::Build(BuildArgs {
        emit: Some(PathBuf::from("out.ninja")),
        targets: vec!["a".into()],
    }),
    ..CliCase::default()
})]
#[case(CliCase {
    argv: vec!["netsuke", "manifest", "out.ninja"],
    expected_cmd: Commands::Manifest {
        file: PathBuf::from("out.ninja"),
    },
    ..CliCase::default()
})]
#[case(CliCase {
    argv: vec!["netsuke", "manifest", "-"],
    expected_cmd: Commands::Manifest {
        file: PathBuf::from("-"),
    },
    ..CliCase::default()
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
    allow_scheme: vec![String::from("http")],
    allow_host: vec!["example.com"],
    block_host: vec!["deny.test"],
    default_deny: true,
    ..CliCase::default()
})]
fn parse_cli(#[case] case: CliCase) -> Result<()> {
    let cli = Cli::try_parse_from(case.argv.clone())
        .context("parse CLI arguments")?
        .with_default_command();
    ensure!(cli.file == case.file, "parsed file should match input");
    ensure!(
        cli.directory == case.directory,
        "parsed directory should match input",
    );
    ensure!(cli.jobs == case.jobs, "parsed jobs should match input");
    ensure!(
        cli.verbose == case.verbose,
        "verbose flag should match input",
    );
    ensure!(
        cli.fetch_allow_scheme == case.allow_scheme,
        "allow-scheme flags should match input",
    );
    let expected_allow_host = case
        .allow_host
        .iter()
        .map(|pattern| {
            HostPattern::parse(pattern)
                .with_context(|| format!("parse expected allow host '{pattern}'"))
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        cli.fetch_allow_host == expected_allow_host,
        "allow-host flags should match input",
    );
    let expected_block_host = case
        .block_host
        .iter()
        .map(|pattern| {
            HostPattern::parse(pattern)
                .with_context(|| format!("parse expected block host '{pattern}'"))
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        cli.fetch_block_host == expected_block_host,
        "block-host flags should match input",
    );
    ensure!(
        cli.fetch_default_deny == case.default_deny,
        "default-deny flag should match input",
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
fn cli_network_policy_defaults_to_https() -> Result<()> {
    let cli = Cli::default();
    let policy = cli.network_policy()?;
    let https = Url::parse("https://example.com").expect("parse https URL");
    let http = Url::parse("http://example.com").expect("parse http URL");
    ensure!(
        policy.evaluate(&https).is_ok(),
        "HTTPS should be permitted by default",
    );
    let err = policy
        .evaluate(&http)
        .expect_err("HTTP should be rejected by default");
    match err {
        NetworkPolicyViolation::SchemeNotAllowed { scheme } => {
            ensure!(scheme == "http", "unexpected scheme {scheme}");
        }
        other => bail!("expected scheme violation, got {other:?}"),
    }
    Ok(())
}

#[rstest]
fn cli_network_policy_default_deny_blocks_unknown_hosts() -> Result<()> {
    let mut cli = Cli {
        fetch_default_deny: true,
        ..Cli::default()
    };
    cli.fetch_allow_host
        .push(HostPattern::parse("example.com").context("parse allow host pattern")?);
    let policy = cli.network_policy()?;
    let allowed = Url::parse("https://example.com").expect("parse allowed URL");
    let denied = Url::parse("https://unauthorised.test").expect("parse denied URL");
    ensure!(
        policy.evaluate(&allowed).is_ok(),
        "explicit allowlist should permit matching host",
    );
    let err = policy
        .evaluate(&denied)
        .expect_err("default deny should block other hosts");
    match err {
        NetworkPolicyViolation::HostNotAllowlisted { host } => {
            ensure!(host == "unauthorised.test", "unexpected host {host}");
        }
        other => bail!("expected allowlist violation, got {other:?}"),
    }
    Ok(())
}

#[rstest]
fn cli_network_policy_blocklist_overrides_allowlist() -> Result<()> {
    let mut cli = Cli::default();
    cli.fetch_allow_host
        .push(HostPattern::parse("example.com").context("parse allow host pattern")?);
    cli.fetch_block_host
        .push(HostPattern::parse("example.com").context("parse block host pattern")?);
    let policy = cli.network_policy()?;
    let url = Url::parse("https://example.com").expect("parse conflicting URL");
    let err = policy
        .evaluate(&url)
        .expect_err("blocklist should override allowlist");
    let err_text = err.to_string();
    match err {
        NetworkPolicyViolation::HostBlocked { host } => {
            ensure!(host == "example.com", "unexpected host {host}");
            ensure!(
                err_text == "host 'example.com' is blocked",
                "unexpected error text: {err_text}",
            );
        }
        other => bail!("expected blocklist violation, got {other:?}"),
    }
    Ok(())
}

#[rstest]
fn cli_network_policy_rejects_invalid_scheme() {
    let mut cli = Cli::default();
    cli.fetch_allow_scheme.push(String::from("1http"));
    let err = cli
        .network_policy()
        .expect_err("invalid scheme should be rejected");
    assert!(
        err.to_string().contains("invalid characters"),
        "unexpected error text: {err}",
    );
}

#[rstest]
#[case(vec!["netsuke", "unknowncmd"], ErrorKind::InvalidSubcommand)]
#[case(vec!["netsuke", "--file"], ErrorKind::InvalidValue)]
#[case(
    vec!["netsuke", "--fetch-allow-host", "bad host"],
    ErrorKind::ValueValidation
)]
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
