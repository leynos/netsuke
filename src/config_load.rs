//! Configuration-load orchestration and its elapsed-time seam.
//!
//! This module keeps the measured startup interval bounded by diagnostic-mode
//! resolution and configuration merging while allowing tests to supply a
//! deterministic elapsed time.

use clap::ArgMatches;
use monotony::MonotonicClock;
use netsuke::cli;

use super::{
    DiagMode, StartupWriter, merge_cli_or_exit, record_config_load_metrics,
    resolve_json_mode_or_exit, settle_startup_diagnostics,
};

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
