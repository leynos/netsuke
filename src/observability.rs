//! Process-level configuration observability.
//!
//! This module owns the bounded metric vocabulary and application recorder used
//! at Netsuke's CLI boundary. Configuration loading remains a plain query; the
//! composition root records its outcomes and duration around that query.

use metrics::{counter, describe_counter, describe_histogram, histogram};
use metrics_util::debugging::{DebuggingRecorder, Snapshotter};
use ortho_config::OrthoError;
use std::{
    error::Error,
    sync::{Once, OnceLock},
    time::Instant,
};

/// Counter recording configuration-load outcomes by bounded phase and outcome.
pub(crate) const CONFIG_LOAD_COUNTER: &str = "config_load_total";
/// Histogram recording configuration-load duration in seconds by bounded phase.
pub(crate) const CONFIG_LOAD_DURATION: &str = "config_load_duration_seconds";
/// Label value for the diagnostic-mode configuration resolution phase.
pub(crate) const DIAG_MODE_PHASE: &str = "diag_mode";
/// Label value for the full configuration merge phase.
pub(crate) const MERGE_PHASE: &str = "merge";
/// Structured-log operation for diagnostic-mode configuration resolution.
pub(crate) const DIAG_MODE_OPERATION: &str = "diag_mode_resolution";
/// Structured-log operation for full configuration merging.
pub(crate) const MERGE_OPERATION: &str = "config_merge";

static SNAPSHOTTER: OnceLock<Snapshotter> = OnceLock::new();

/// Install the process metrics recorder once the tracing subscriber is ready.
///
/// The binary owns global recorder installation. Tests use local recorders so
/// their samples stay isolated from each other and the process-wide recorder.
pub(crate) fn init_metrics() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    if recorder.install().is_ok() {
        drop(SNAPSHOTTER.set(snapshotter));
    }
}

/// Emit the recorder's drained aggregate at process shutdown.
pub(crate) fn emit_metrics_snapshot() {
    if let Some(snapshotter) = SNAPSHOTTER.get() {
        tracing::debug!(metrics = ?snapshotter.snapshot().into_vec(), "metrics snapshot");
    }
}

/// Record the outcome and duration of one configuration-loading phase.
pub(crate) fn record_config_load<T, E>(
    phase: &'static str,
    load: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    describe_config_metrics();
    let started = Instant::now();
    let result = load();
    let outcome = if result.is_ok() { "success" } else { "failure" };
    counter!(CONFIG_LOAD_COUNTER, "phase" => phase, "outcome" => outcome).increment(1);
    histogram!(CONFIG_LOAD_DURATION, "phase" => phase).record(started.elapsed());
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
mod tests {
    //! Regression coverage for bounded configuration observability.

    use super::*;
    use metrics::{SharedString, Unit};
    use metrics_util::{
        CompositeKey, MetricKind,
        debugging::{DebugValue, DebuggingRecorder},
    };

    type SnapshotEntry = (CompositeKey, Option<Unit>, Option<SharedString>, DebugValue);

    fn assert_one_counter_record(snapshot: &[SnapshotEntry], phase: &str, outcome: &str) {
        assert_eq!(
            snapshot
                .iter()
                .filter(|entry| is_counter_record(entry, phase, outcome))
                .count(),
            1,
            "expected one configuration-load counter record for phase {phase} and outcome {outcome}",
        );
    }

    fn is_counter_record(entry: &SnapshotEntry, phase: &str, outcome: &str) -> bool {
        entry.0.kind() == MetricKind::Counter
            && entry.0.key().name() == CONFIG_LOAD_COUNTER
            && has_label(entry, phase)
            && has_label(entry, outcome)
            && matches!(entry.3, DebugValue::Counter(1))
    }

    fn has_label(entry: &SnapshotEntry, value: &str) -> bool {
        entry.0.key().labels().any(|label| label.value() == value)
    }

    fn count_single_sample_duration_records(snapshot: &[SnapshotEntry]) -> usize {
        snapshot
            .iter()
            .filter(|entry| is_single_sample_duration_record(entry))
            .count()
    }

    fn is_single_sample_duration_record(entry: &SnapshotEntry) -> bool {
        entry.0.kind() == MetricKind::Histogram
            && entry.0.key().name() == CONFIG_LOAD_DURATION
            && matches!(entry.3, DebugValue::Histogram(ref values) if values.len() == 1)
    }

    #[test]
    fn classifies_config_errors_without_exposing_details() {
        let file_error = OrthoError::File {
            path: "private.toml".into(),
            source: Box::new(std::io::Error::other("read failure")),
        };
        let validation_error = OrthoError::Validation {
            key: "jobs".into(),
            message: "must be positive".into(),
        };
        let fallback_error = std::io::Error::other("unknown source");

        assert_eq!(classify_error(&file_error), "io");
        assert_eq!(classify_error(&validation_error), "validation");
        assert_eq!(classify_error(&fallback_error), "parse");
    }

    #[test]
    fn records_each_config_load_phase_and_outcome() {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();

        metrics::with_local_recorder(&recorder, || {
            let success = record_config_load(DIAG_MODE_PHASE, || Ok::<_, ()>(()));
            let failure = record_config_load(MERGE_PHASE, || Err::<(), _>(()));

            assert!(success.is_ok());
            assert!(failure.is_err());
        });

        let snapshot = snapshotter.snapshot().into_vec();
        assert_one_counter_record(&snapshot, DIAG_MODE_PHASE, "success");
        assert_one_counter_record(&snapshot, MERGE_PHASE, "failure");
        assert_eq!(count_single_sample_duration_records(&snapshot), 2);
    }
}
