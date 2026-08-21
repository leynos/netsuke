//! Process-recorder coverage for startup and phase configuration metrics.
//!
//! These tests exercise the application-owned [`ConfigMetricsRecorder`] that
//! `main` installs globally. They verify its snapshot retains only the bounded
//! startup and phase series, so verbose shutdown diagnostics do not discard
//! the aggregate `netsuke_config_load_*` metrics or retain unrelated workload
//! observations.

use super::*;
use metrics::{SharedString, Unit};
use metrics_util::{CompositeKey, MetricKind, debugging::DebugValue};

type SnapshotEntry = (CompositeKey, Option<Unit>, Option<SharedString>, DebugValue);

fn assert_retained_startup_counter(snapshot: &[SnapshotEntry]) {
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Counter
                && entry.0.key().name() == STARTUP_CONFIG_LOAD_COUNTER
                && matches!(
                    entry.0.key().labels().collect::<Vec<_>>().as_slice(),
                    [label] if label.key() == "outcome" && label.value() == "failure"
                )
                && matches!(entry.3, DebugValue::Counter(1))
        }),
        "snapshot should retain the bounded startup counter: {snapshot:?}"
    );
}

fn assert_retained_startup_duration(snapshot: &[SnapshotEntry]) {
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Histogram
                && entry.0.key().name() == STARTUP_CONFIG_LOAD_DURATION
                && entry.0.key().labels().next().is_none()
                && matches!(entry.3, DebugValue::Histogram(ref values) if values.as_slice() == [0.02])
        }),
        "snapshot should retain the unlabelled startup duration: {snapshot:?}"
    );
}

fn assert_only_configuration_metric_names(snapshot: &[SnapshotEntry]) {
    assert!(
        snapshot.iter().all(|entry| {
            matches!(
                entry.0.key().name(),
                CONFIG_LOAD_COUNTER
                    | CONFIG_LOAD_DURATION
                    | STARTUP_CONFIG_LOAD_COUNTER
                    | STARTUP_CONFIG_LOAD_DURATION
            )
        }),
        "snapshot must reject unrelated metric series: {snapshot:?}"
    );
}

#[test]
fn configuration_metrics_recorder_retains_bounded_startup_and_phase_series() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        counter!(CONFIG_LOAD_COUNTER, "phase" => DIAG_MODE_PHASE, "outcome" => "success")
            .increment(1);
        histogram!(CONFIG_LOAD_DURATION, "phase" => DIAG_MODE_PHASE).record(0.01);
        counter!(STARTUP_CONFIG_LOAD_COUNTER, "outcome" => "failure").increment(1);
        histogram!(STARTUP_CONFIG_LOAD_DURATION).record(0.02);
        counter!(STARTUP_CONFIG_LOAD_COUNTER).increment(1);
        histogram!(STARTUP_CONFIG_LOAD_DURATION, "phase" => DIAG_MODE_PHASE).record(0.03);
        counter!("help_targets_total", "topic" => "help").increment(1);
        histogram!("template_render_duration_seconds", "template" => "unbounded").record(0.5);
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        4,
        "only bounded configuration metric series should be retained"
    );
    assert_retained_startup_counter(&snapshot);
    assert_retained_startup_duration(&snapshot);
    assert_only_configuration_metric_names(&snapshot);
}
