//! Configuration-load orchestration and its elapsed-time seam.
//!
//! This module keeps the measured startup interval bounded by diagnostic-mode
//! resolution and configuration merging while allowing tests to supply a
//! deterministic elapsed time.

use clap::ArgMatches;
use monotony::MonotonicClock;
use netsuke::cli;
use std::{
    io::{self, Write},
    process::ExitCode,
    time::Duration,
};

use super::{
    DiagMode, StartupWriter, diagnostic_json, observability, set_tracing_filter,
    settle_startup_diagnostics, startup_filter,
};

/// Counter recording the outcome of each configuration-load attempt.
///
/// Labelled by `outcome` (`success` or `failure`) so operators can track the
/// startup configuration-load failure rate.
const CONFIG_LOAD_TOTAL: &str = "netsuke_config_load_total";

/// Histogram recording the elapsed duration of the configuration-load
/// phase (diagnostic-mode resolution through layer merge) in seconds.
const CONFIG_LOAD_DURATION_SECONDS: &str = "netsuke_config_load_duration_seconds";

/// Cached diagnostic resolution carried into the full configuration merge.
struct ResolvedDiagnosticMode {
    mode: DiagMode,
    discovered_layers: cli::DiscoveredLayers,
}

/// Dependencies that define one configuration-load attempt.
///
/// This context is private to startup orchestration; it groups the parsed
/// inputs that must remain within the measured configuration-load interval.
pub(super) struct ConfigurationLoadContext<'a> {
    parsed_cli: &'a cli::Cli,
    matches: &'a ArgMatches,
    startup_mode: DiagMode,
    startup_writer: &'a StartupWriter,
}

impl<'a> ConfigurationLoadContext<'a> {
    /// Construct the input context for one configuration-load attempt.
    pub(super) const fn new(
        parsed_cli: &'a cli::Cli,
        matches: &'a ArgMatches,
        startup_mode: DiagMode,
        startup_writer: &'a StartupWriter,
    ) -> Self {
        Self {
            parsed_cli,
            matches,
            startup_mode,
            startup_writer,
        }
    }
}

/// Resolve diagnostic mode and merge configuration while recording one metric.
///
/// The measured interval starts immediately before diagnostic-mode resolution
/// and ends after either that resolution or the full configuration merge.
pub(super) fn resolve_configuration(
    context: &ConfigurationLoadContext<'_>,
    clock: &impl MonotonicClock,
) -> Result<cli::Cli, std::process::ExitCode> {
    let started_at = clock.now();
    let resolution = match resolve_json_mode_or_exit(
        context.parsed_cli,
        context.matches,
        context.startup_mode,
        clock,
    ) {
        Ok(mode) => mode,
        Err(code) => {
            record_config_load_metrics(clock.now().duration_since(started_at), false);
            settle_startup_diagnostics(context.startup_writer, context.startup_mode);
            return Err(code);
        }
    };
    // The effective mode is known here, before configuration is merged, so the
    // startup warning reaches the user ahead of any configuration processing.
    settle_startup_diagnostics(context.startup_writer, resolution.mode);
    let merged_cli = match merge_cli_or_exit(context.parsed_cli, context.matches, resolution, clock)
    {
        Ok(merged) => merged,
        Err(code) => {
            record_config_load_metrics(clock.now().duration_since(started_at), false);
            return Err(code);
        }
    };
    record_config_load_metrics(clock.now().duration_since(started_at), true);
    Ok(merged_cli)
}

/// Report a configuration error and return its failure [`ExitCode`].
///
/// JSON mode emits a valid, stable diagnostic document with its serialization
/// fallback. Human mode logs only bounded `operation` and `error_category`
/// fields structurally before writing the user-facing error to stderr.
pub(super) fn config_err_to_exit(
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
            "configuration load failed"
        );
        drop(writeln!(io::stderr(), "{err}"));
        ExitCode::FAILURE
    }
}

/// Resolve JSON mode and cache discovered configuration layers for the merge.
///
/// The diagnostic-mode phase is timed with the injected [`MonotonicClock`].
/// Resolution failures select the fallback mode's filter and return a failure
/// [`ExitCode`] through [`config_err_to_exit`].
fn resolve_json_mode_or_exit(
    parsed_cli: &cli::Cli,
    matches: &ArgMatches,
    fallback_mode: DiagMode,
    clock: &impl MonotonicClock,
) -> Result<ResolvedDiagnosticMode, ExitCode> {
    match observability::record_config_load(observability::ConfigLoadPhase::DiagMode, clock, || {
        let (result, outcome) = cli::resolve_json_and_layers_outcome_with_env(
            parsed_cli,
            matches,
            &cli::ConfigStdEnvProvider,
        );
        match result {
            Ok(is_json_enabled) => Ok((is_json_enabled, outcome)),
            Err(error) => Err(Box::new((error, outcome))),
        }
    }) {
        Ok((is_json_enabled, outcome)) => {
            let mode = DiagMode::from_json_enabled(is_json_enabled);
            set_tracing_filter(startup_filter(mode, parsed_cli.verbose));
            outcome.emit_diagnostics();
            Ok(ResolvedDiagnosticMode {
                mode,
                discovered_layers: outcome.into_layers(),
            })
        }
        Err(error_and_outcome) => {
            let (err, outcome) = *error_and_outcome;
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

/// Merge CLI values with the layers cached during diagnostic-mode resolution.
///
/// The merge is timed with the injected [`MonotonicClock`], applies the default
/// command, and maps configuration failures to a failure [`ExitCode`].
fn merge_cli_or_exit(
    parsed_cli: &cli::Cli,
    matches: &ArgMatches,
    resolution: ResolvedDiagnosticMode,
    clock: &impl MonotonicClock,
) -> Result<cli::Cli, ExitCode> {
    observability::record_config_load(observability::ConfigLoadPhase::Merge, clock, || {
        cli::merge_with_cached_file_layers(
            parsed_cli,
            matches,
            &cli::ConfigStdEnvProvider,
            resolution.discovered_layers,
        )
    })
    .map(cli::Cli::with_default_command)
    .map_err(|err| {
        config_err_to_exit(
            err.as_ref(),
            resolution.mode,
            observability::MERGE_OPERATION,
        )
    })
}

/// Emit the configuration-load metrics for one startup attempt.
///
/// Recording goes through the `metrics` façade, backed by the application's
/// in-process `DebuggingRecorder`.
fn record_config_load_metrics(elapsed: Duration, succeeded: bool) {
    let outcome = if succeeded { "success" } else { "failure" };
    metrics::histogram!(CONFIG_LOAD_DURATION_SECONDS).record(elapsed.as_secs_f64());
    metrics::counter!(CONFIG_LOAD_TOTAL, "outcome" => outcome).increment(1);
}

#[cfg(test)]
#[path = "config_load_metrics_tests.rs"]
mod config_load_metrics_tests;
