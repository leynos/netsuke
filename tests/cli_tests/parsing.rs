//! CLI parsing coverage.

use anyhow::{Context, Result, ensure};
use clap::error::ErrorKind;
use netsuke::cli::{
    AccessibilityPolicy, BuildArgs, Cli, ColourPolicy, Commands, EmojiPolicy, ProgressPolicy,
};
use netsuke::cli_localization;
use netsuke::host_pattern::HostPattern;
use netsuke::output_mode::OutputMode;
use netsuke::output_prefs;
use netsuke::theme::ThemeContext;
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
    json: bool,
    allow_scheme: Vec<String>,
    allow_host: Vec<&'static str>,
    block_host: Vec<&'static str>,
    default_deny: bool,
    color: ColourPolicy,
    emoji: EmojiPolicy,
    progress: ProgressPolicy,
    accessibility: AccessibilityPolicy,
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
            json: false,
            allow_scheme: Vec::new(),
            allow_host: Vec::new(),
            block_host: Vec::new(),
            default_deny: false,
            color: ColourPolicy::Auto,
            emoji: EmojiPolicy::Auto,
            progress: ProgressPolicy::Auto,
            accessibility: AccessibilityPolicy::Auto,
            default_targets: Vec::new(),
            expected_cmd: Commands::Build(BuildArgs {
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
    argv: vec!["netsuke", "--progress", "never"],
    progress: ProgressPolicy::Never,
    ..CliCase::default()
})]
#[case(CliCase {
    argv: vec!["netsuke", "--emoji", "auto"],
    emoji: EmojiPolicy::Auto,
    ..CliCase::default()
})]
#[case(CliCase {
    argv: vec!["netsuke", "--emoji", "never"],
    emoji: EmojiPolicy::Never,
    ..CliCase::default()
})]
#[case(CliCase {
    argv: vec!["netsuke", "--emoji", "always"],
    emoji: EmojiPolicy::Always,
    ..CliCase::default()
})]
#[case(CliCase {
    argv: vec!["netsuke", "--color", "always"],
    color: ColourPolicy::Always,
    ..CliCase::default()
})]
#[case(CliCase {
    argv: vec!["netsuke", "--accessibility", "on"],
    accessibility: AccessibilityPolicy::On,
    ..CliCase::default()
})]
#[case(CliCase {
    argv: vec!["netsuke", "--json"],
    json: true,
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
    argv: vec!["netsuke", "generate", "--output", "out.ninja"],
    expected_cmd: Commands::Generate {
        output: Some(PathBuf::from("out.ninja")),
    },
    ..CliCase::default()
})]
#[case(CliCase {
    argv: vec!["netsuke", "generate"],
    expected_cmd: Commands::Generate {
        output: None,
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
    ensure!(cli.json == case.json, "json flag should match input",);
    ensure!(cli.no_input(), "no-input should remain enabled");
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
    ensure!(cli.color == case.color, "color policy should match input",);
    ensure!(cli.emoji == case.emoji, "emoji policy should match input",);
    ensure!(
        cli.progress == case.progress,
        "progress policy should match input",
    );
    ensure!(
        cli.accessibility == case.accessibility,
        "accessibility policy should match input",
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
#[case(vec!["netsuke", "--locale", "nope"], ErrorKind::ValueValidation)]
#[case(vec!["netsuke", "--color", "loud"], ErrorKind::ValueValidation)]
#[case(vec!["netsuke", "--emoji", "sometimes"], ErrorKind::ValueValidation)]
#[case(vec!["netsuke", "--progress", "paused"], ErrorKind::ValueValidation)]
#[case(vec!["netsuke", "--accessibility", "yes"], ErrorKind::ValueValidation)]
#[case(vec!["netsuke", "--diag-json"], ErrorKind::UnknownArgument)]
#[case(vec!["netsuke", "--colour-policy", "always"], ErrorKind::UnknownArgument)]
#[case(vec!["netsuke", "--spinner-mode", "enabled"], ErrorKind::UnknownArgument)]
#[case(vec!["netsuke", "--accessible", "true"], ErrorKind::UnknownArgument)]
#[case(vec!["netsuke", "--no-emoji"], ErrorKind::UnknownArgument)]
#[case(vec!["netsuke", "--output-format", "json"], ErrorKind::UnknownArgument)]
#[case(vec!["netsuke", "--theme", "ascii"], ErrorKind::UnknownArgument)]
#[case(vec!["netsuke", "--progress"], ErrorKind::InvalidValue)]
#[case(vec!["netsuke", "build", "--emit", "out.ninja"], ErrorKind::UnknownArgument)]
#[case(vec!["netsuke", "manifest", "-"], ErrorKind::InvalidSubcommand)]
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

#[test]
fn emoji_never_forces_ascii_output() -> Result<()> {
    let cli = Cli {
        emoji: EmojiPolicy::Never,
        ..Cli::default()
    };

    let prefs = output_prefs::resolve_from_theme_with(
        cli.theme_preference(),
        ThemeContext::new(None, Some(cli.color), OutputMode::Standard),
        |_| None,
    );

    ensure!(
        !prefs.emoji_allowed(),
        "emoji = never should force ASCII output",
    );
    Ok(())
}

#[test]
fn emoji_always_forces_unicode_output() -> Result<()> {
    let cli = Cli {
        emoji: EmojiPolicy::Always,
        ..Cli::default()
    };

    let prefs = output_prefs::resolve_from_theme_with(
        cli.theme_preference(),
        ThemeContext::new(None, Some(cli.color), OutputMode::Standard),
        |_| None,
    );

    ensure!(
        prefs.emoji_allowed(),
        "emoji = always should force Unicode output",
    );
    Ok(())
}
