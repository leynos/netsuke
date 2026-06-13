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
use ortho_config::{Localizer, OrthoError};
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::Level;
use tracing_subscriber::fmt;

/// Counter recording the outcome of each configuration-load phase.
///
/// Labelled by `phase` (`diag_mode` or `merge`) and `outcome` (`success` or
/// `failure`) so operators can track the startup configuration-load failure
/// rate per phase.
const CONFIG_LOAD_TOTAL: &str = "netsuke_config_load_total";

/// Histogram recording the wall-clock duration of each configuration-load
/// phase in seconds, labelled by `phase` (`diag_mode` or `merge`).
const CONFIG_LOAD_DURATION_SECONDS: &str = "netsuke_config_load_duration_seconds";

/// The `diag_mode` configuration-load phase: diagnostic-mode resolution.
const PHASE_DIAG_MODE: &str = "diag_mode";

/// The `merge` configuration-load phase: full layer merge.
const PHASE_MERGE: &str = "merge";

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
    let (mode, merge_result) = resolve_configuration(&parsed_cli, &matches);
    let merged_cli = match merge_result {
        Ok(merged) => merged.with_default_command(),
        Err(err) => return handle_config_load_error(&err, mode),
    };
    let runtime_mode = DiagMode::from_json_enabled(merged_cli.diag_json);
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

/// Resolve the diagnostic mode and merge configuration layers, recording
/// startup-latency and outcome metrics for the combined config-load phase.
///
/// The returned [`DiagMode`] is needed to render any merge failure, so it is
/// produced even when the merge fails. The histogram spans diagnostic-mode
/// resolution through the layer merge, and the counter is labelled by the
/// merge outcome.
fn resolve_configuration(
    parsed_cli: &cli::Cli,
    matches: &ArgMatches,
) -> (DiagMode, Result<cli::Cli, Arc<OrthoError>>) {
    let diag_start = Instant::now();
    let mode = DiagMode::from_json_enabled(cli::resolve_merged_diag_json(parsed_cli, matches));
    // Diagnostic-mode resolution has an internal fallback and cannot fail at
    // this boundary, so its phase outcome is always `success`.
    record_config_load_phase(PHASE_DIAG_MODE, diag_start.elapsed(), true);

    let merge_start = Instant::now();
    let merged = cli::merge_with_config(parsed_cli, matches);
    record_config_load_phase(PHASE_MERGE, merge_start.elapsed(), merged.is_ok());
    (mode, merged)
}

/// Emit the configuration-load metrics for one phase of one startup attempt.
///
/// Recording goes through the `metrics` façade, so it is a no-op unless the
/// operator has installed a recorder.
fn record_config_load_phase(phase: &'static str, elapsed: Duration, succeeded: bool) {
    let outcome = if succeeded { "success" } else { "failure" };
    metrics::histogram!(CONFIG_LOAD_DURATION_SECONDS, "phase" => phase)
        .record(elapsed.as_secs_f64());
    metrics::counter!(CONFIG_LOAD_TOTAL, "phase" => phase, "outcome" => outcome).increment(1);
}

/// Coarse classification of a configuration-load error for log correlation.
///
/// Groups [`OrthoError`] variants into operational categories so operators can
/// filter failures by cause without depending on the full error text.
const fn error_category(err: &OrthoError) -> &'static str {
    match err {
        OrthoError::CliParsing(_) | OrthoError::Gathering(_) => "parse",
        OrthoError::File { .. } => "io",
        OrthoError::CyclicExtends { .. }
        | OrthoError::Merge { .. }
        | OrthoError::Validation { .. } => "validation",
        OrthoError::Aggregate(_) => "aggregate",
        _ => "other",
    }
}

/// Render a configuration-load failure to the user and map it to an exit code.
///
/// Any error reaching this point comes from the `config_merge` phase
/// (diagnostic-mode resolution cannot fail at the call boundary), so the
/// structured log records `operation = "config_merge"` alongside a coarse
/// `error_category`.
fn handle_config_load_error(err: &Arc<OrthoError>, mode: DiagMode) -> ExitCode {
    if mode.is_json() {
        diagnostic_json::emit_or_fallback(diagnostic_json::render_error_json(err.as_ref()))
    } else {
        init_tracing(Level::ERROR);
        tracing::error!(
            error = %err,
            operation = "config_merge",
            error_category = error_category(err),
            "configuration load failed"
        );
        ExitCode::FAILURE
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

#[cfg(test)]
mod tests {
    use super::*;
    use metrics::Label;
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    /// Capture the metrics emitted while `body` runs against a local recorder.
    fn captured_metrics(body: impl FnOnce()) -> Vec<(String, Vec<Label>, DebugValue)> {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        metrics::with_local_recorder(&recorder, body);
        snapshotter
            .snapshot()
            .into_vec()
            .into_iter()
            .map(|(composite, _unit, _desc, value)| {
                let key = composite.key();
                (
                    key.name().to_owned(),
                    key.labels().cloned().collect::<Vec<_>>(),
                    value,
                )
            })
            .collect()
    }

    fn has_label(labels: &[Label], key: &str, value: &str) -> bool {
        labels
            .iter()
            .any(|label| label.key() == key && label.value() == value)
    }

    #[test]
    fn record_config_load_phase_counts_failure_outcome_with_phase_label() {
        let metrics = captured_metrics(|| {
            record_config_load_phase(PHASE_MERGE, Duration::from_millis(5), false);
        });

        let counter = metrics
            .iter()
            .find(|(name, _, _)| name == CONFIG_LOAD_TOTAL)
            .expect("config-load counter should be recorded");
        assert!(
            has_label(&counter.1, "phase", "merge") && has_label(&counter.1, "outcome", "failure"),
            "counter should carry phase=merge, outcome=failure: {:?}",
            counter.1
        );
        assert_eq!(counter.2, DebugValue::Counter(1));

        let histogram = metrics
            .iter()
            .find(|(name, _, _)| name == CONFIG_LOAD_DURATION_SECONDS)
            .expect("config-load duration histogram should be recorded");
        assert!(
            has_label(&histogram.1, "phase", "merge"),
            "histogram should carry phase=merge: {:?}",
            histogram.1
        );
        let DebugValue::Histogram(samples) = &histogram.2 else {
            panic!("expected a histogram value, got {:?}", histogram.2);
        };
        assert_eq!(samples.len(), 1, "exactly one duration sample expected");
    }

    #[test]
    fn record_config_load_phase_counts_success_outcome() {
        let metrics = captured_metrics(|| {
            record_config_load_phase(PHASE_DIAG_MODE, Duration::from_millis(1), true);
        });
        let counter = metrics
            .iter()
            .find(|(name, _, _)| name == CONFIG_LOAD_TOTAL)
            .expect("config-load counter should be recorded");
        assert!(
            has_label(&counter.1, "phase", "diag_mode")
                && has_label(&counter.1, "outcome", "success"),
            "counter should carry phase=diag_mode, outcome=success: {:?}",
            counter.1
        );
    }

    #[test]
    fn error_category_classifies_ortho_error_variants() {
        assert_eq!(
            error_category(&OrthoError::Validation {
                key: "jobs".into(),
                message: "out of range".into(),
            }),
            "validation"
        );
        assert_eq!(
            error_category(&OrthoError::File {
                path: std::path::PathBuf::from("netsuke.toml"),
                source: Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "missing",)),
            }),
            "io"
        );
        assert_eq!(
            error_category(&OrthoError::CyclicExtends {
                cycle: "a -> b -> a".into(),
            }),
            "validation"
        );
    }
}
