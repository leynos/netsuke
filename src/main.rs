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
mod observability;
#[path = "startup_tracing.rs"]
mod startup_tracing;

use startup_tracing::StartupWriter;

/// Send buffered startup diagnostics where `mode` says they belong.
///
/// Human mode releases them to stderr; JSON mode drops them, so the diagnostic
/// document is the only thing on that stream.
fn settle_startup_diagnostics(writer: &StartupWriter, mode: DiagMode) {
    if mode.is_json() {
        writer.discard();
    } else if let Err(err) = writer.release_to_stderr() {
        // Nothing better to do: the channel for reporting this is the one that
        // just failed.
        drop(writeln!(
            io::stderr(),
            "failed to flush startup diagnostics: {err}"
        ));
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
    env: &impl locale_resolution::LocaleEnvProvider,
    system_locale: &impl locale_resolution::SystemLocale,
) -> ExitCode {
    let json_hint = locale_resolution::resolve_startup_json(&args, env);
    // Recorded at `WARN` but written to a buffer, not to stderr. `json_hint` is
    // only a hint — configuration can still turn JSON on — and the JSON
    // diagnostic goes to stderr, so an event emitted now could corrupt it.
    // Buffering keeps the locale fallback report without taking that risk.
    let startup_writer = StartupWriter::buffering();
    init_tracing(LevelFilter::WARN, startup_writer.clone());
    observability::init_metrics();
    let localizer = startup_localizer(&args, env, system_locale);
    let startup_mode = DiagMode::from_json_enabled(json_hint);
    let (parsed_cli, matches) =
        match parse_cli_or_exit(args, &localizer, startup_mode, &startup_writer) {
            Ok(parsed) => parsed,
            // The buffer was settled inside, before the branch that exits.
            Err(code) => return code,
        };
    let verbose = parsed_cli.verbose;
    if is_informational_help(&parsed_cli) {
        settle_startup_diagnostics(&startup_writer, startup_mode);
        return finish_run(run_cli(&parsed_cli, system_locale, startup_mode), verbose);
    }

    let (mode, discovered_layers) =
        match resolve_json_mode_or_exit(&parsed_cli, &matches, startup_mode) {
            Ok(resolved) => resolved,
            Err(code) => {
                settle_startup_diagnostics(&startup_writer, startup_mode);
                return finish_run(code, verbose);
            }
        };
    // The effective mode is known here, before configuration is merged, so the
    // startup warning reaches the user ahead of any configuration processing.
    settle_startup_diagnostics(&startup_writer, mode);
    let merged_cli = match merge_cli_or_exit(&parsed_cli, &matches, mode, discovered_layers) {
        Ok(merged) => merged,
        Err(code) => return finish_run(code, verbose),
    };
    let merged_verbose = merged_cli.verbose;
    let runtime_mode = DiagMode::from_json_enabled(merged_cli.json);
    finish_run(
        run_cli(&merged_cli, system_locale, runtime_mode),
        merged_verbose,
    )
}

/// Emit a development snapshot after the command has completed when requested.
fn finish_run(exit_code: ExitCode, verbose: bool) -> ExitCode {
    if verbose {
        observability::emit_metrics_snapshot();
    }
    exit_code
}

const fn is_informational_help(cli: &cli::Cli) -> bool {
    matches!(
        &cli.command,
        Some(cli::Commands::Help(args))
            if !matches!(args.topic.as_ref(), Some(cli::HelpTopic::Targets))
    )
}

fn run_cli(
    cli: &cli::Cli,
    system_locale: &impl locale_resolution::SystemLocale,
    runtime_mode: DiagMode,
) -> ExitCode {
    configure_runtime(cli, system_locale, runtime_mode);
    let output_mode = output_mode::resolve(cli.accessibility_override(), Some(cli.color));
    let prefs = output_prefs::resolve_from_theme(
        cli.theme_preference(),
        ThemeContext::new(None, Some(cli.color), output_mode),
    );
    match runner::run(cli, prefs) {
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
/// traced at that level.
///
/// Otherwise `WARN`: a run that falls back to English, or loads a catalogue
/// that fails to parse, reports it at that level, and `ERROR` would leave both
/// silent — which is the condition a user would report as a bug.
const fn startup_filter(mode: DiagMode, verbose: bool) -> LevelFilter {
    if mode.is_json() {
        LevelFilter::OFF
    } else if verbose {
        LevelFilter::TRACE
    } else {
        LevelFilter::WARN
    }
}

/// Install the process-wide subscriber with a reloadable level filter set to
/// `initial`.
///
/// Only the first call installs; later calls are ignored so exactly one global
/// subscriber exists, and the level is adjusted through [`set_tracing_filter`]
/// rather than by installing a second subscriber. Events go to `writer`, which
/// buffers until the effective mode is known and then releases to stderr or
/// discards — never to stdout, so a JSON document is never interleaved.
fn init_tracing(initial: LevelFilter, writer: StartupWriter) {
    let (filter, handle) = reload::Layer::new(initial);
    if Registry::default()
        .with(filter)
        .with(
            fmt::layer()
                .with_writer(writer)
                // Colour only a terminal; piped or redirected logs stay plain so
                // they remain greppable and free of escape sequences.
                .with_ansi(io::stderr().is_terminal()),
        )
        .try_init()
        .is_ok()
    {
        TRACING_FILTER.set(handle).ok();
    }
}

/// Adjust the installed subscriber's level, if one was installed.
fn set_tracing_filter(level: LevelFilter) {
    if let Some(handle) = TRACING_FILTER.get() {
        handle.modify(|filter| *filter = level).ok();
    }
}

fn startup_localizer(
    args: &[OsString],
    env: &impl locale_resolution::LocaleEnvProvider,
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
    startup_writer: &StartupWriter,
) -> Result<(cli::Cli, ArgMatches), ExitCode> {
    match cli::parse_with_localizer_from(args, localizer) {
        Ok(parsed) => Ok(parsed),
        Err(err) => {
            // Every arm below terminates the process or returns, and
            // `Error::exit` never returns, so the buffered startup
            // diagnostics have to be settled here. Configuration is never
            // read on these paths, so `mode` is the effective mode.
            settle_startup_diagnostics(startup_writer, mode);
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

fn config_err_to_exit(
    err: &(dyn std::error::Error + 'static),
    mode: DiagMode,
    operation: &'static str,
) -> ExitCode {
    if mode.is_json() {
        diagnostic_json::emit_or_fallback(diagnostic_json::render_error_json(err))
    } else {
        tracing::error!(
            operation,
            error_category = observability::classify_error(err),
            error = %err,
            "configuration load failed"
        );
        ExitCode::FAILURE
    }
}

fn resolve_json_mode_or_exit(
    parsed_cli: &cli::Cli,
    matches: &ArgMatches,
    fallback_mode: DiagMode,
) -> Result<(DiagMode, cli::DiscoveredLayers), ExitCode> {
    let (result, outcome) = cli::resolve_json_and_layers_outcome_with_env(
        parsed_cli,
        matches,
        &cli::ConfigStdEnvProvider,
    );
    match observability::record_config_load(observability::ConfigLoadPhase::DiagMode, || result) {
        Ok(is_json_enabled) => {
            let mode = DiagMode::from_json_enabled(is_json_enabled);
            set_tracing_filter(startup_filter(mode, parsed_cli.verbose));
            outcome.emit_diagnostics();
            Ok((mode, outcome.into_layers()))
        }
        Err(err) => {
            let fallback_filter = startup_filter(fallback_mode, parsed_cli.verbose);
            set_tracing_filter(fallback_filter);
            outcome.emit_diagnostics();
            Err(config_err_to_exit(
                err.as_ref(),
                fallback_mode,
                observability::DIAG_MODE_OPERATION,
            ))
        }
    }
}

fn merge_cli_or_exit(
    parsed_cli: &cli::Cli,
    matches: &ArgMatches,
    mode: DiagMode,
    discovered_layers: cli::DiscoveredLayers,
) -> Result<cli::Cli, ExitCode> {
    observability::record_config_load(observability::ConfigLoadPhase::Merge, || {
        cli::merge_with_cached_file_layers(
            parsed_cli,
            matches,
            &cli::ConfigStdEnvProvider,
            discovered_layers,
        )
    })
    .map(cli::Cli::with_default_command)
    .map_err(|err| config_err_to_exit(err.as_ref(), mode, observability::MERGE_OPERATION))
}

fn configure_runtime(
    merged_cli: &cli::Cli,
    system_locale: &impl locale_resolution::SystemLocale,
    mode: DiagMode,
) {
    // Raised before the localizer is built, so a fallback warning is both
    // visible in a normal run and suppressed in JSON mode, where stderr
    // carries the diagnostic document.
    set_tracing_filter(startup_filter(mode, merged_cli.verbose));

    let runtime_locale = locale_resolution::resolve_runtime_locale(merged_cli, system_locale);
    let runtime_localizer = Arc::from(cli_localization::build_localizer(runtime_locale.as_deref()));
    localization::set_localizer(Arc::clone(&runtime_localizer));
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

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
