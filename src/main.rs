//! Application entry point.
//!
//! Parses command-line arguments and delegates execution to [`runner::run`].

use clap::ArgMatches;
use clap::error::ErrorKind;
use miette::Report;
use netsuke::theme::ThemeContext;
use netsuke::{
    cli, cli_localization, diagnostic_json, locale_resolution, localization, manifest, output_mode,
    output_prefs, runner,
};
use ortho_config::Localizer;
use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;
use std::sync::{Arc, OnceLock};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{Registry, fmt, reload};

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
    let json_hint = locale_resolution::resolve_startup_json(&args, env);
    let localizer = startup_localizer(&args, env, system_locale);
    let startup_mode = DiagMode::from_json_enabled(json_hint);
    let (parsed_cli, matches) = match parse_cli_or_exit(args, &localizer, startup_mode) {
        Ok(parsed) => parsed,
        Err(code) => return code,
    };
    // Install the one global subscriber before configuration is resolved, so the
    // selector and environment-lookup events emitted during resolution are
    // recorded. The startup JSON hint gates it: JSON mode must put nothing but
    // the diagnostic document on stderr.
    init_tracing(startup_filter(startup_mode, parsed_cli.verbose));

    let mode = match resolve_json_mode_or_exit(&parsed_cli, &matches, startup_mode) {
        Ok(mode) => mode,
        Err(code) => return code,
    };
    // Re-apply once the resolved mode is known, in case a config file enabled
    // JSON when the raw arguments did not.
    set_tracing_filter(startup_filter(mode, parsed_cli.verbose));

    let merged_cli = match merge_cli_or_exit(&parsed_cli, &matches, mode) {
        Ok(merged) => merged,
        Err(code) => return code,
    };
    let runtime_mode = DiagMode::from_json_enabled(merged_cli.json);
    configure_runtime(&merged_cli, system_locale, runtime_mode);
    let output_mode =
        output_mode::resolve(merged_cli.accessibility_override(), Some(merged_cli.color));
    let prefs = output_prefs::resolve_from_theme(
        merged_cli.theme_preference(),
        ThemeContext::new(None, Some(merged_cli.color), output_mode),
    );
    match runner::run(&merged_cli, prefs) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => handle_runner_error(err, prefs, runtime_mode),
    }
}

/// Handle for adjusting the installed subscriber's level after startup.
static TRACING_FILTER: OnceLock<reload::Handle<LevelFilter, Registry>> = OnceLock::new();

/// Choose the stderr verbosity for `mode`.
///
/// JSON mode silences tracing entirely so stderr carries only the diagnostic
/// document. `--verbose` selects `TRACE` because the `NETSUKE_CONFIG` lookup is
/// traced at that level; otherwise only errors surface.
fn startup_filter(mode: DiagMode, verbose: bool) -> LevelFilter {
    if mode.is_json() {
        LevelFilter::OFF
    } else if verbose {
        LevelFilter::TRACE
    } else {
        LevelFilter::ERROR
    }
}

/// Install the process-wide subscriber with a reloadable level filter.
///
/// Only the first call installs; later calls are ignored so exactly one global
/// subscriber exists, and the level is adjusted through [`set_tracing_filter`]
/// rather than by installing a second subscriber.
fn init_tracing(initial: LevelFilter) {
    let (filter, handle) = reload::Layer::new(initial);
    if Registry::default()
        .with(filter)
        .with(
            fmt::layer()
                .with_writer(io::stderr)
                // Colour only a terminal; piped or redirected logs stay plain so
                // they remain greppable and free of escape sequences.
                .with_ansi(io::stderr().is_terminal()),
        )
        .try_init()
        .is_ok()
    {
        let _ = TRACING_FILTER.set(handle);
    }
}

/// Adjust the installed subscriber's level, if one was installed.
fn set_tracing_filter(level: LevelFilter) {
    if let Some(handle) = TRACING_FILTER.get() {
        let _ = handle.modify(|filter| *filter = level);
    }
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

fn config_err_to_exit(err: &(dyn std::error::Error + 'static), mode: DiagMode) -> ExitCode {
    if mode.is_json() {
        diagnostic_json::emit_or_fallback(diagnostic_json::render_error_json(err))
    } else {
        tracing::error!(error = %err, "configuration load failed");
        ExitCode::FAILURE
    }
}

fn resolve_json_mode_or_exit(
    parsed_cli: &cli::Cli,
    matches: &ArgMatches,
    fallback_mode: DiagMode,
) -> Result<DiagMode, ExitCode> {
    cli::resolve_merged_json(parsed_cli, matches)
        .map(DiagMode::from_json_enabled)
        .map_err(|err| config_err_to_exit(err.as_ref(), fallback_mode))
}

fn merge_cli_or_exit(
    parsed_cli: &cli::Cli,
    matches: &ArgMatches,
    mode: DiagMode,
) -> Result<cli::Cli, ExitCode> {
    cli::merge_with_config(parsed_cli, matches)
        .map(cli::Cli::with_default_command)
        .map_err(|err| config_err_to_exit(err.as_ref(), mode))
}

fn configure_runtime(
    merged_cli: &cli::Cli,
    system_locale: &impl locale_resolution::SystemLocale,
    mode: DiagMode,
) {
    let runtime_locale = locale_resolution::resolve_runtime_locale(merged_cli, system_locale);
    let runtime_localizer = Arc::from(cli_localization::build_localizer(runtime_locale.as_deref()));
    localization::set_localizer(Arc::clone(&runtime_localizer));

    set_tracing_filter(startup_filter(mode, merged_cli.verbose));
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
