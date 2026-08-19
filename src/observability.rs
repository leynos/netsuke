//! Process-level configuration observability.
//!
//! This module owns the bounded metric vocabulary and application recorder used
//! at Netsuke's CLI boundary. Configuration loading remains a plain query; the
//! composition root records its outcomes and duration around that query.

use metrics::{counter, describe_counter, describe_histogram, histogram};
use metrics_util::debugging::Snapshotter;
use monotony::MonotonicClock;
use ortho_config::OrthoError;
use std::{
    error::Error,
    sync::{Once, OnceLock},
};

#[path = "observability_recorder.rs"]
mod recorder;

use self::recorder::ConfigMetricsRecorder;

/// Counter recording configuration-load outcomes by bounded phase and outcome.
pub(crate) const CONFIG_LOAD_COUNTER: &str = "config_load_total";
/// Histogram recording configuration-load duration in seconds by bounded phase.
pub(crate) const CONFIG_LOAD_DURATION: &str = "config_load_duration_seconds";
/// Counter recording the outcome of a complete startup configuration load.
pub(crate) const STARTUP_CONFIG_LOAD_COUNTER: &str = "netsuke_config_load_total";
/// Histogram recording the duration of a complete startup configuration load.
pub(crate) const STARTUP_CONFIG_LOAD_DURATION: &str = "netsuke_config_load_duration_seconds";
/// Label value for the diagnostic-mode configuration resolution phase.
pub(crate) const DIAG_MODE_PHASE: &str = "diag_mode";
/// Label value for the full configuration merge phase.
pub(crate) const MERGE_PHASE: &str = "merge";
/// Structured-log operation for diagnostic-mode configuration resolution.
pub(crate) const DIAG_MODE_OPERATION: &str = "diag_mode_resolution";
/// Structured-log operation for full configuration merging.
pub(crate) const MERGE_OPERATION: &str = "config_merge";

/// Bounded configuration-loading phase used in metrics labels.
#[derive(Clone, Copy)]
pub(crate) enum ConfigLoadPhase {
    /// Resolve diagnostic JSON mode from configuration.
    DiagMode,
    /// Merge all configuration layers.
    Merge,
}

impl ConfigLoadPhase {
    /// Return the stable metric label for this phase.
    const fn as_label(self) -> &'static str {
        match self {
            Self::DiagMode => DIAG_MODE_PHASE,
            Self::Merge => MERGE_PHASE,
        }
    }
}

/// Bounded configuration-loading outcome used in metrics labels.
#[derive(Clone, Copy)]
pub(crate) enum ConfigLoadOutcome {
    /// The phase completed successfully.
    Success,
    /// The phase returned an error.
    Failure,
}

impl ConfigLoadOutcome {
    /// Classify a configuration-loading result without retaining its error.
    const fn from_is_ok(is_ok: bool) -> Self {
        if is_ok { Self::Success } else { Self::Failure }
    }

    /// Return the stable metric label for this outcome.
    const fn as_label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
        }
    }
}

/// Process-global metrics snapshotter, installed when metrics initialise.
static SNAPSHOTTER: OnceLock<Snapshotter> = OnceLock::new();
/// Guards one-time installation of the process metrics recorder.
static METRICS_INITIALIZED: Once = Once::new();

/// Install the process metrics recorder once the tracing subscriber is ready.
///
/// The binary owns global recorder installation. Tests use local recorders so
/// their samples stay isolated from each other and the process-wide recorder.
pub(crate) fn init_metrics() {
    METRICS_INITIALIZED.call_once(|| {
        let recorder = ConfigMetricsRecorder::new();
        let snapshotter = recorder.snapshotter();
        if metrics::set_global_recorder(recorder).is_ok() {
            drop(SNAPSHOTTER.set(snapshotter));
        }
    });
}

/// Emit the recorder's drained aggregate at process shutdown.
pub(crate) fn emit_metrics_snapshot() {
    if let Some(snapshotter) = SNAPSHOTTER.get() {
        tracing::debug!(metrics = ?snapshotter.snapshot().into_vec(), "metrics snapshot");
    }
}

/// Record the outcome and duration of one configuration-loading phase.
pub(crate) fn record_config_load<T, E>(
    phase: ConfigLoadPhase,
    clock: &impl MonotonicClock,
    load: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    describe_config_metrics();
    let started = clock.now();
    let result = load();
    let outcome = ConfigLoadOutcome::from_is_ok(result.is_ok());
    counter!(
        CONFIG_LOAD_COUNTER,
        "phase" => phase.as_label(),
        "outcome" => outcome.as_label()
    )
    .increment(1);
    histogram!(CONFIG_LOAD_DURATION, "phase" => phase.as_label())
        .record(clock.now().duration_since(started));
    result
}

/// Classify a configuration error without exposing its path or display text.
pub(crate) fn classify_error(err: &(dyn Error + 'static)) -> &'static str {
    match err.downcast_ref::<OrthoError>() {
        Some(OrthoError::File { .. }) => "io",
        Some(OrthoError::Validation { .. }) => "validation",
        _ => "parse",
    }
}

/// Describe the stable configuration metrics once per process.
fn describe_config_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            CONFIG_LOAD_COUNTER,
            "Counts configuration-load outcomes by bounded phase and outcome."
        );
        describe_histogram!(
            CONFIG_LOAD_DURATION,
            "Measures configuration-load duration in seconds by bounded phase."
        );
    });
}

#[cfg(test)]
#[path = "observability_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "observability_recorder_tests.rs"]
mod recorder_tests;
