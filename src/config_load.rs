//! Configuration-load orchestration and its elapsed-time seam.
//!
//! This module keeps the measured startup interval bounded by diagnostic-mode
//! resolution and configuration merging while allowing tests to supply a
//! deterministic elapsed time.

use clap::ArgMatches;
use netsuke::cli;
use std::time::{Duration, Instant};

use super::{
    DiagMode, StartupWriter, merge_cli_or_exit, record_config_load_metrics,
    resolve_json_mode_or_exit, settle_startup_diagnostics,
};

/// Supplies elapsed time for one configuration-load attempt.
pub(super) trait ConfigurationLoadClock {
    /// Begin measuring a configuration-load attempt.
    fn restart(&mut self);

    /// Return the duration since the most recent [`Self::restart`] call.
    fn elapsed(&self) -> Duration;
}

/// Wall-clock implementation used by the production startup path.
pub(super) struct SystemConfigurationLoadClock {
    started: Instant,
}

impl SystemConfigurationLoadClock {
    /// Construct a clock ready to measure its first configuration load.
    pub(super) fn new() -> Self {
        Self {
            started: Instant::now(),
        }
    }
}

impl ConfigurationLoadClock for SystemConfigurationLoadClock {
    fn restart(&mut self) {
        self.started = Instant::now();
    }

    fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }
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
    clock: &mut impl ConfigurationLoadClock,
) -> Result<cli::Cli, std::process::ExitCode> {
    clock.restart();
    let (mode, discovered_layers) = match resolve_json_mode_or_exit(
        context.parsed_cli,
        context.matches,
        context.startup_mode,
    ) {
        Ok(mode) => mode,
        Err(code) => {
            record_config_load_metrics(clock.elapsed(), false);
            settle_startup_diagnostics(context.startup_writer, context.startup_mode);
            return Err(code);
        }
    };
    // The effective mode is known here, before configuration is merged, so the
    // startup warning reaches the user ahead of any configuration processing.
    settle_startup_diagnostics(context.startup_writer, mode);
    let merged_cli =
        match merge_cli_or_exit(context.parsed_cli, context.matches, mode, discovered_layers) {
            Ok(merged) => merged,
            Err(code) => {
                record_config_load_metrics(clock.elapsed(), false);
                return Err(code);
            }
        };
    record_config_load_metrics(clock.elapsed(), true);
    Ok(merged_cli)
}
