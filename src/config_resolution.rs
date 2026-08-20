//! Configuration-resolution orchestration at the CLI composition root.
//!
//! Configuration loading runs as two timed phases that share one discovery
//! pass: resolving the diagnostic JSON preference and merging the full
//! configuration. This module times each phase through the observable
//! recorder, replays retained discovery diagnostics only after tracing is
//! configured, and maps configuration errors to process exit codes. Startup
//! ownership — the [`DiagMode`] classification and the tracing-filter helpers
//! — stays in the crate root; this module imports only what it needs.

use crate::cli;
use crate::diagnostic_json;
use crate::observability;
use crate::{DiagMode, set_tracing_filter, startup_filter};
use clap::ArgMatches;
use monotony::MonotonicClock;
use std::process::ExitCode;

/// Map a configuration-load failure to a process exit code.
///
/// JSON mode emits a diagnostic document; human mode records a bounded
/// structured event naming only the bounded operation and error category,
/// never the error's paths or display text.
pub(crate) fn config_err_to_exit(
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
        ExitCode::FAILURE
    }
}

/// Resolve diagnostic JSON mode and its shared layers as one timed phase.
pub(crate) fn resolve_json_mode_or_exit(
    parsed_cli: &cli::Cli,
    matches: &ArgMatches,
    fallback_mode: DiagMode,
) -> Result<(DiagMode, cli::DiscoveredLayers), ExitCode> {
    let env = cli::ConfigStdEnvProvider;
    let clock = monotony::StdMonotonicClock;
    JsonModeResolutionContext {
        parsed_cli,
        matches,
        fallback_mode,
        env: &env,
        clock: &clock,
    }
    .resolve_with(cli::resolve_json_and_layers_outcome_with_env)
}

/// Startup dependencies for one timed diagnostic-mode resolution.
///
/// The fields are crate-visible so the dependency-injection boundary test can
/// drive a timed resolution with a fixed clock and environment.
pub(crate) struct JsonModeResolutionContext<'a, E, C> {
    pub(crate) parsed_cli: &'a cli::Cli,
    pub(crate) matches: &'a ArgMatches,
    pub(crate) fallback_mode: DiagMode,
    pub(crate) env: &'a E,
    pub(crate) clock: &'a C,
}

impl<E, C> JsonModeResolutionContext<'_, E, C>
where
    E: cli::ConfigEnvProvider,
    C: MonotonicClock,
{
    /// Resolve JSON mode and replay retained diagnostics after filtering.
    pub(crate) fn resolve_with<R>(
        self,
        resolver: R,
    ) -> Result<(DiagMode, cli::DiscoveredLayers), ExitCode>
    where
        R: FnOnce(
            &cli::Cli,
            &ArgMatches,
            &E,
        ) -> (ortho_config::OrthoResult<bool>, cli::DiscoveryOutcome),
    {
        let Self {
            parsed_cli,
            matches,
            fallback_mode,
            env,
            clock,
        } = self;
        match observability::record_config_load(
            observability::ConfigLoadPhase::DiagMode,
            clock,
            || {
                let (result, outcome) = resolver(parsed_cli, matches, env);
                match result {
                    Ok(is_json_enabled) => Ok((is_json_enabled, outcome)),
                    Err(error) => Err(Box::new((error, outcome))),
                }
            },
        ) {
            Ok((is_json_enabled, outcome)) => {
                let mode = DiagMode::from_json_enabled(is_json_enabled);
                set_tracing_filter(startup_filter(mode, parsed_cli.verbose));
                outcome.emit_diagnostics();
                Ok((mode, outcome.into_layers()))
            }
            Err(error) => {
                let (err, outcome) = *error;
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
}

/// Merge the full configuration from already-discovered layers, timed.
pub(crate) fn merge_cli_or_exit(
    parsed_cli: &cli::Cli,
    matches: &ArgMatches,
    mode: DiagMode,
    discovered_layers: cli::DiscoveredLayers,
) -> Result<cli::Cli, ExitCode> {
    let clock = monotony::StdMonotonicClock;
    observability::record_config_load(observability::ConfigLoadPhase::Merge, &clock, || {
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
