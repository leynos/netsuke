//! Unit tests for CLI argument parsing and validation.
//!
//! This module exercises the command-line interface defined in [`netsuke::cli`]
//! using `rstest` for parameterised coverage of success and error scenarios.

use anyhow::{Context, Result, bail, ensure};
use clap::error::ErrorKind;
use clap::{CommandFactory, FromArgMatches, Parser};
use netsuke::cli::{BuildArgs, Cli, Commands, locale_hint_from_args};
use netsuke::cli_localization;
use netsuke::host_pattern::HostPattern;
use netsuke::stdlib::NetworkPolicyViolation;
use ortho_config::{CliValueExtractor, MergeComposer, sanitize_value};
use rstest::rstest;
use serde_json::json;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use url::Url;

struct EnvGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
        let previous = std::env::var_os(key);
        // SAFETY: Tests run in a single process and the guard restores values.
        unsafe { std::env::set_var(key, value.as_ref()) };
        Self { key, previous }
    }

    fn unset(key: &'static str) -> Self {
        let previous = std::env::var_os(key);
        // SAFETY: Tests run in a single process and the guard restores values.
        unsafe { std::env::remove_var(key) };
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            // SAFETY: Restores the previous environment value for the test.
            unsafe { std::env::set_var(self.key, previous) };
        } else {
            // SAFETY: Restores the previous environment state for the test.
            unsafe { std::env::remove_var(self.key) };
        }
    }
}

struct CliCase {
    argv: Vec<&'static str>,
    file: PathBuf,
    directory: Option<PathBuf>,
    jobs: Option<usize>,
    verbose: bool,
    locale: Option<&'static str>,
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
            locale: None,
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
    argv: vec!["netsuke", "--locale", "es-ES"],
    locale: Some("es-ES"),
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
        cli.locale.as_deref() == case.locale,
        "locale should match input",
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
#[case(vec!["netsuke", "--locale", "nope"], ErrorKind::ValueValidation)]
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

fn os_args(args: &[&str]) -> Vec<OsString> {
    args.iter().map(|arg| OsString::from(*arg)).collect()
}

#[rstest]
fn locale_hint_from_args_accepts_space_form() -> Result<()> {
    let args = os_args(&["netsuke", "--locale", "es-ES"]);
    let hint = locale_hint_from_args(&args);
    ensure!(
        hint.as_deref() == Some("es-ES"),
        "expected Some(\"es-ES\"), got: {hint:?}"
    );
    Ok(())
}

#[rstest]
fn locale_hint_from_args_accepts_equals_form() -> Result<()> {
    let args = os_args(&["netsuke", "--locale=es-ES"]);
    let hint = locale_hint_from_args(&args);
    ensure!(
        hint.as_deref() == Some("es-ES"),
        "expected Some(\"es-ES\"), got: {hint:?}"
    );
    Ok(())
}

#[rstest]
fn locale_hint_from_args_trailing_locale_flag_yields_none() -> Result<()> {
    let args = os_args(&["netsuke", "--locale"]);
    let hint = locale_hint_from_args(&args);
    ensure!(
        hint.is_none(),
        "expected None for trailing --locale without value, got: {hint:?}"
    );
    Ok(())
}

#[rstest]
fn locale_hint_from_args_ignores_args_after_double_dash() -> Result<()> {
    let args = os_args(&["netsuke", "--verbose", "--", "--locale", "es-ES"]);
    let hint = locale_hint_from_args(&args);
    ensure!(
        hint.is_none(),
        "expected None when --locale appears after \"--\", got: {hint:?}"
    );
    Ok(())
}

#[rstest]
fn locale_hint_from_args_uses_last_locale_flag() -> Result<()> {
    let args = os_args(&["netsuke", "--locale", "es-ES", "--locale", "en-US"]);
    let hint = locale_hint_from_args(&args);
    ensure!(
        hint.as_deref() == Some("en-US"),
        "expected last --locale to win (\"en-US\"), got: {hint:?}"
    );
    Ok(())
}

#[rstest]
fn cli_extract_user_provided_omits_defaults() -> Result<()> {
    let mut matches = Cli::command()
        .try_get_matches_from(["netsuke"])
        .context("parse matches for default CLI")?;
    let cli = Cli::from_arg_matches_mut(&mut matches).context("build CLI from matches")?;
    let value = cli
        .extract_user_provided(&matches)
        .context("extract CLI overrides")?;
    let object = value
        .as_object()
        .context("expected extracted CLI value to be an object")?;
    ensure!(
        !object.contains_key("file"),
        "default file should not be treated as a CLI override",
    );
    ensure!(
        !object.contains_key("verbose"),
        "default verbose flag should not be treated as a CLI override",
    );
    ensure!(
        !object.contains_key("fetch_default_deny"),
        "default deny flag should not be treated as a CLI override",
    );
    Ok(())
}

#[rstest]
fn cli_merge_layers_respects_precedence_and_appends_lists() -> Result<()> {
    let mut composer = MergeComposer::new();
    let mut defaults = sanitize_value(&Cli::default())?;
    let defaults_object = defaults
        .as_object_mut()
        .context("defaults should be an object")?;
    defaults_object.insert("jobs".to_owned(), json!(1));
    defaults_object.insert("fetch_allow_scheme".to_owned(), json!(["https"]));
    composer.push_defaults(defaults);
    composer.push_file(
        json!({
            "file": "Configfile",
            "jobs": 2,
            "fetch_allow_scheme": ["http"],
            "locale": "en-US"
        }),
        None,
    );
    composer.push_environment(json!({
        "jobs": 3,
        "fetch_allow_scheme": ["ftp"]
    }));
    composer.push_cli(json!({
        "jobs": 4,
        "fetch_allow_scheme": ["git"],
        "verbose": true
    }));
    let merged = Cli::merge_from_layers(composer.layers())?;
    ensure!(
        merged.file.as_path() == Path::new("Configfile"),
        "file layer should override defaults",
    );
    ensure!(merged.jobs == Some(4), "CLI layer should override jobs",);
    ensure!(
        merged.fetch_allow_scheme == vec!["https", "http", "ftp", "git"],
        "list values should append in layer order",
    );
    ensure!(
        merged.locale.as_deref() == Some("en-US"),
        "file layer should populate locale when CLI does not override",
    );
    ensure!(merged.verbose, "CLI layer should set verbose");
    Ok(())
}

#[rstest]
fn cli_merge_with_config_respects_precedence_and_skips_empty_cli_layer() -> Result<()> {
    let temp_dir = tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    let config = r#"
jobs = 2
fetch_allow_scheme = ["https"]
"#;
    fs::write(&config_path, config).context("write netsuke.toml")?;

    let _config_guard = EnvGuard::set("NETSUKE_CONFIG_PATH", config_path.as_os_str());
    let _jobs_guard = EnvGuard::set("NETSUKE_JOBS", "4");
    let _scheme_guard = EnvGuard::unset("NETSUKE_FETCH_ALLOW_SCHEME");

    let localizer = cli_localization::build_localizer(None);
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(["netsuke"], localizer.as_ref())
        .context("parse CLI args for merge")?;
    let merged = netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge CLI and configuration layers")?
        .with_default_command();

    ensure!(
        merged.jobs == Some(4),
        "environment variables should override config when CLI has no value",
    );
    ensure!(
        merged.fetch_allow_scheme == vec!["https".to_owned()],
        "config values should apply when CLI overrides are empty",
    );

    Ok(())
}

#[rstest]
fn cli_localises_invalid_subcommand_in_spanish() -> Result<()> {
    let localizer = cli_localization::build_localizer(Some("es-ES"));
    let err = netsuke::cli::parse_with_localizer_from(
        ["netsuke", "--locale", "es-ES", "unknown"],
        localizer.as_ref(),
    )
    .err()
    .context("parser should reject invalid subcommand")?;
    ensure!(
        err.to_string().contains("Subcomando desconocido"),
        "expected Spanish localisation, got: {err}",
    );
    Ok(())
}
