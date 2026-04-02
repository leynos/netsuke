//! CLI parsing coverage.

use anyhow::{Context, Result, ensure};
use clap::error::ErrorKind;
use netsuke::cli::config::{ColourPolicy, OutputFormat, SpinnerMode};
use netsuke::cli::{BuildArgs, Commands};
use netsuke::cli_localization;
use netsuke::host_pattern::HostPattern;
use netsuke::theme::ThemePreference;
use rstest::rstest;
use std::path::PathBuf;
use std::sync::Arc;

struct CliCase {
    argv: Vec<&'static str>,
    file: PathBuf,
    directory: Option<PathBuf>,
    jobs: Option<usize>,
    verbose: bool,
    locale: Option<&'static str>,
    diag_json: bool,
    allow_scheme: Vec<String>,
    allow_host: Vec<&'static str>,
    block_host: Vec<&'static str>,
    default_deny: bool,
    progress: Option<bool>,
    theme: Option<ThemePreference>,
    colour_policy: Option<ColourPolicy>,
    spinner_mode: Option<SpinnerMode>,
    output_format: Option<OutputFormat>,
    default_targets: Vec<String>,
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
            locale: None,
            diag_json: false,
            allow_scheme: Vec::new(),
            allow_host: Vec::new(),
            block_host: Vec::new(),
            default_deny: false,
            progress: None,
            theme: None,
            colour_policy: None,
            spinner_mode: None,
            output_format: None,
            default_targets: Vec::new(),
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
    argv: vec!["netsuke", "--progress", "false"],
    progress: Some(false),
    ..CliCase::default()
})]
#[case(CliCase {
    argv: vec!["netsuke", "--theme", "auto"],
    theme: Some(ThemePreference::Auto),
    ..CliCase::default()
})]
#[case(CliCase {
    argv: vec!["netsuke", "--theme", "ascii"],
    theme: Some(ThemePreference::Ascii),
    ..CliCase::default()
})]
#[case(CliCase {
    argv: vec!["netsuke", "--theme", "unicode"],
    theme: Some(ThemePreference::Unicode),
    ..CliCase::default()
})]
#[case(CliCase {
    argv: vec!["netsuke", "--colour-policy", "always"],
    colour_policy: Some(ColourPolicy::Always),
    ..CliCase::default()
})]
#[case(CliCase {
    argv: vec!["netsuke", "--spinner-mode", "disabled"],
    spinner_mode: Some(SpinnerMode::Disabled),
    ..CliCase::default()
})]
#[case(CliCase {
    argv: vec!["netsuke", "--output-format", "json"],
    output_format: Some(OutputFormat::Json),
    ..CliCase::default()
})]
#[case(CliCase {
    argv: vec!["netsuke", "--default-target", "lint", "--default-target", "test"],
    default_targets: vec![String::from("lint"), String::from("test")],
    ..CliCase::default()
})]
#[case(CliCase {
    argv: vec!["netsuke", "--locale", "es-ES"],
    locale: Some("es-ES"),
    ..CliCase::default()
})]
#[case(CliCase {
    argv: vec!["netsuke", "--diag-json"],
    diag_json: true,
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
    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (parsed_cli, _) = netsuke::cli::parse_with_localizer_from(case.argv.clone(), &localizer)
        .context("parse CLI arguments")?;
    let cli = parsed_cli.with_default_command();
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
        cli.locale.as_deref() == case.locale,
        "locale should match input",
    );
    ensure!(
        cli.diag_json == case.diag_json,
        "diag_json flag should match input",
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
    ensure!(
        cli.progress == case.progress,
        "progress flag should match input",
    );
    ensure!(cli.theme == case.theme, "theme flag should match input");
    ensure!(
        cli.colour_policy == case.colour_policy,
        "colour_policy flag should match input",
    );
    ensure!(
        cli.spinner_mode == case.spinner_mode,
        "spinner_mode flag should match input",
    );
    ensure!(
        cli.output_format == case.output_format,
        "output_format flag should match input",
    );
    ensure!(
        cli.default_targets == case.default_targets,
        "default-target flags should match input",
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
#[case(
    vec!["netsuke", "--fetch-allow-host", "bad host"],
    ErrorKind::ValueValidation
)]
#[case(vec!["netsuke", "-j", "notanumber"], ErrorKind::ValueValidation)]
#[case(vec!["netsuke", "--file", "alt.yml", "-C"], ErrorKind::InvalidValue)]
#[case(vec!["netsuke", "manifest"], ErrorKind::MissingRequiredArgument)]
#[case(vec!["netsuke", "--locale", "nope"], ErrorKind::ValueValidation)]
#[case(vec!["netsuke", "--theme", "neon"], ErrorKind::ValueValidation)]
#[case(vec!["netsuke", "--colour-policy", "loud"], ErrorKind::ValueValidation)]
#[case(vec!["netsuke", "--spinner-mode", "paused"], ErrorKind::ValueValidation)]
#[case(vec!["netsuke", "--output-format", "tap"], ErrorKind::ValueValidation)]
fn parse_cli_errors(#[case] argv: Vec<&str>, #[case] expected_error: ErrorKind) -> Result<()> {
    let localizer = Arc::from(cli_localization::build_localizer(None));
    let err = netsuke::cli::parse_with_localizer_from(argv, &localizer)
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
