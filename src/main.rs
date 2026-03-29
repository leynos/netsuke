//! Application entry point.
//!
//! Parses command-line arguments and delegates execution to [`runner::run`].

use clap::ArgMatches;
use clap::error::ErrorKind;
use miette::Report;
use netsuke::{
    cli, cli_localization, diagnostic_json, locale_resolution, localization, manifest, output_mode,
    output_prefs, runner, theme::ThemeContext,
};
use ortho_config::Localizer;
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;
use std::sync::Arc;
use tracing::Level;
use tracing_subscriber::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiagMode {
    Human,
    Json,
}

impl DiagMode {
    const fn from_json_enabled(enabled: bool) -> Self {
        if enabled { Self::Json } else { Self::Human }
    }

    const fn is_json(self) -> bool {
        matches!(self, Self::Json)
    }
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    let env = locale_resolution::SystemEnv;
    let system_locale = locale_resolution::SysLocale;
    run_with_args(args, &env, &system_locale)
}

fn run_with_args(
    args: Vec<OsString>,
    env: &impl locale_resolution::EnvProvider,
    system_locale: &impl locale_resolution::SystemLocale,
) -> ExitCode {
    let diag_json_hint = locale_resolution::resolve_startup_diag_json(&args, env);
    let localizer = startup_localizer(&args, env, system_locale);
    let startup_mode = DiagMode::from_json_enabled(diag_json_hint);
    let (parsed_cli, matches) = match parse_cli_or_exit(args, &localizer, startup_mode) {
        Ok(parsed) => parsed,
        Err(code) => return code,
    };
    let mode = DiagMode::from_json_enabled(cli::resolve_merged_diag_json(&parsed_cli, &matches));

    let merged_cli = match merge_cli_or_exit(&parsed_cli, &matches, mode) {
        Ok(merged) => merged,
        Err(code) => return code,
    };
    let runtime_mode = DiagMode::from_json_enabled(merged_cli.resolved_diag_json());
    configure_runtime(&merged_cli, system_locale, runtime_mode);
    let output_mode = output_mode::resolve(merged_cli.accessible, merged_cli.colour_policy);
    let prefs = output_prefs::resolve_from_theme(
        merged_cli.theme,
        ThemeContext::new(merged_cli.no_emoji, merged_cli.colour_policy, output_mode),
    );
    match runner::run(&merged_cli, prefs) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => handle_runner_error(err, prefs, runtime_mode),
    }
}

fn init_tracing(max_level: Level) {
    fmt()
        .with_max_level(max_level)
        .with_writer(io::stderr)
        .init();
}

fn startup_localizer(
    args: &[OsString],
    env: &impl locale_resolution::EnvProvider,
    system_locale: &impl locale_resolution::SystemLocale,
) -> Arc<dyn Localizer> {
    let startup_locale = locale_resolution::resolve_startup_locale(args, env, system_locale);
    let localizer = Arc::from(cli_localization::build_localizer(startup_locale.as_deref()));
    localization::set_localizer(Arc::clone(&localizer));
    localizer
}

fn parse_cli_or_exit(
    args: Vec<OsString>,
    localizer: &Arc<dyn Localizer>,
    mode: DiagMode,
) -> Result<(cli::Cli, ArgMatches), ExitCode> {
    match cli::parse_with_localizer_from(args, localizer) {
        Ok(parsed) => Ok(parsed),
        Err(err) => {
            if matches!(
                err.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                err.exit();
            }
            if mode.is_json() {
                Err(diagnostic_json::emit_or_fallback(
                    diagnostic_json::render_error_json(&err),
                ))
            } else {
                err.exit();
            }
        }
    }
}

fn merge_cli_or_exit(
    parsed_cli: &cli::Cli,
    matches: &ArgMatches,
    mode: DiagMode,
) -> Result<cli::Cli, ExitCode> {
    match cli::merge_with_config(parsed_cli, matches) {
        Ok(merged) => Ok(merged.with_default_command()),
        Err(err) => {
            if mode.is_json() {
                Err(diagnostic_json::emit_or_fallback(
                    diagnostic_json::render_error_json(err.as_ref()),
                ))
            } else {
                init_tracing(Level::ERROR);
                tracing::error!(error = %err, "configuration load failed");
                Err(ExitCode::FAILURE)
            }
        }
    }
}

fn configure_runtime(
    merged_cli: &cli::Cli,
    system_locale: &impl locale_resolution::SystemLocale,
    mode: DiagMode,
) {
    let runtime_locale = locale_resolution::resolve_runtime_locale(merged_cli, system_locale);
    let runtime_localizer = Arc::from(cli_localization::build_localizer(runtime_locale.as_deref()));
    localization::set_localizer(Arc::clone(&runtime_localizer));

    if matches!(
        merged_cli.colour_policy,
        Some(cli::config::ColourPolicy::Never)
    ) {
        // Align downstream human-facing libraries with the configured policy.
        unsafe {
            std::env::set_var("NO_COLOR", "1");
        }
    }

    if !mode.is_json() {
        let max_level = if merged_cli.verbose {
            Level::DEBUG
        } else {
            Level::ERROR
        };
        init_tracing(max_level);
    }
}

fn handle_runner_error(
    err: anyhow::Error,
    prefs: output_prefs::OutputPrefs,
    mode: DiagMode,
) -> ExitCode {
    if mode.is_json() {
        return diagnostic_json::emit_or_fallback(render_runtime_error_json(&err));
    }
    let prefix = prefs.error_prefix();
    match err.downcast::<runner::RunnerError>() {
        Ok(runner_err) => {
            let report = Report::new(runner_err);
            drop(writeln!(io::stderr(), "{prefix} {report:?}"));
        }
        Err(other_err) => {
            tracing::error!(error = %other_err, "runner failed");
            drop(writeln!(io::stderr(), "{prefix} {other_err}"));
        }
    }
    ExitCode::FAILURE
}

fn render_runtime_error_json(err: &anyhow::Error) -> serde_json::Result<String> {
    if let Some(runner_err) = err.downcast_ref::<runner::RunnerError>() {
        return diagnostic_json::render_diagnostic_json(runner_err);
    }
    if let Some(manifest_err) = err
        .chain()
        .find_map(|cause| cause.downcast_ref::<manifest::ManifestError>())
    {
        return diagnostic_json::render_diagnostic_json(manifest_err);
    }
    if let Some(report) = err.downcast_ref::<Report>() {
        return diagnostic_json::render_report_json(report);
    }
    diagnostic_json::render_error_json(err.as_ref())
}
