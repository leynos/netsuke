//! Process-recorder coverage for bounded application metrics.
//!
//! These tests exercise the application-owned [`ConfigMetricsRecorder`] that
//! `main` installs globally. They verify its snapshot retains the bounded
//! configuration and timing-summary series while rejecting unrelated workload
//! observations.

use super::*;
use metrics::{SharedString, Unit};
use metrics_util::{CompositeKey, MetricKind, debugging::DebugValue};

type SnapshotEntry = (CompositeKey, Option<Unit>, Option<SharedString>, DebugValue);

/// Record the valid, invalid, and unrelated series used to verify filtering.
fn record_mixed_metric_series(recorder: &ConfigMetricsRecorder) {
    metrics::with_local_recorder(recorder, || {
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
}

/// Assert the retained startup counter has its sole bounded failure label.
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

/// Assert the retained startup duration is unlabelled with one expected sample.
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

/// Assert every retained series belongs to the bounded configuration vocabulary.
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

    record_mixed_metric_series(&recorder);

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

/// The application recorder retains the bounded timing-summary sink series.
#[test]
fn recorder_retains_bounded_timing_summary_sink_series() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        counter!(TIMING_SUMMARY_SINK_WRITES_TOTAL, "outcome" => "success").increment(1);
        counter!(TIMING_SUMMARY_SINK_WRITES_TOTAL, "outcome" => "write_error").increment(1);
        histogram!(TIMING_SUMMARY_SINK_WRITE_DURATION).record(0.01);
        counter!(TIMING_SUMMARY_SINK_WRITES_TOTAL, "outcome" => "unbounded").increment(1);
        histogram!(TIMING_SUMMARY_SINK_WRITE_DURATION, "outcome" => "success").record(0.02);
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        3,
        "only the two bounded timing outcomes and unlabelled duration are retained"
    );
    assert_timing_summary_counter(&snapshot, "success");
    assert_timing_summary_counter(&snapshot, "write_error");
    assert_timing_summary_duration(&snapshot);
}

/// Assert that `outcome` retains one bounded timing-summary counter increment.
fn assert_timing_summary_counter(snapshot: &[SnapshotEntry], outcome: &str) {
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Counter
                && entry.0.key().name() == TIMING_SUMMARY_SINK_WRITES_TOTAL
                && entry.0.key().labels().count() == 1
                && entry
                    .0
                    .key()
                    .labels()
                    .any(|label| label.key() == "outcome" && label.value() == outcome)
                && matches!(entry.3, DebugValue::Counter(1))
        }),
        "expected retained timing-summary counter with outcome {outcome}: {snapshot:?}"
    );
}

/// Assert that the unlabelled timing-summary duration has one sample.
fn assert_timing_summary_duration(snapshot: &[SnapshotEntry]) {
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Histogram
                && entry.0.key().name() == TIMING_SUMMARY_SINK_WRITE_DURATION
                && entry.0.key().labels().next().is_none()
                && matches!(entry.3, DebugValue::Histogram(ref values) if values.as_slice() == [0.01])
        }),
        "expected retained timing-summary duration: {snapshot:?}"
    );
}
