//! Verify production snapshots retain only bounded console-scheduling series.

use super::{ConfigMetricsRecorder, SnapshotEntry};
use metrics::{counter, gauge, histogram};
use metrics_util::{MetricKind, debugging::DebugValue};
use netsuke::runner::{
    BUNDLE_CONSOLE_EDGES_TOTAL, BUNDLE_GENERATION_DURATION, BUNDLE_GENERATIONS_TOTAL,
    NINJA_BUILD_CONSOLE_EDGES_TOTAL, NINJA_BUILD_EXECUTION_DURATION, NINJA_BUILD_EXECUTIONS_TOTAL,
};

/// Emit valid series and adversarial variants through the application recorder.
fn record_console_series() {
    for name in [BUNDLE_GENERATIONS_TOTAL, NINJA_BUILD_EXECUTIONS_TOTAL] {
        counter!(name, "outcome" => "success", "error_category" => "none", "console_scheduling" => "enabled").increment(1);
        counter!(name, "console_scheduling" => "private-output", "outcome" => "success", "error_category" => "none").increment(1);
        counter!(name, "console_scheduling" => "enabled", "outcome" => "success", "error_category" => "none", "output" => "private-output").increment(1);
        counter!(name, "outcome" => "success", "error_category" => "none").increment(1);
        gauge!(name, "console_scheduling" => "enabled", "outcome" => "success", "error_category" => "none").set(1.0);
    }
    for name in [BUNDLE_GENERATION_DURATION, NINJA_BUILD_EXECUTION_DURATION] {
        histogram!(name, "outcome" => "error", "error_category" => if name == BUNDLE_GENERATION_DURATION { "console_pool_conflict" } else { "io" }, "console_scheduling" => "disabled").record(0.25);
        histogram!(name, "console_scheduling" => "disabled", "outcome" => "error", "error_category" => "private-process-error").record(0.25);
    }
    for name in [BUNDLE_CONSOLE_EDGES_TOTAL, NINJA_BUILD_CONSOLE_EDGES_TOTAL] {
        counter!(name, "outcome" => "success").increment(2);
        counter!(name, "outcome" => "success", "edge" => "private-output").increment(2);
        counter!(name, "outcome" => "private-outcome").increment(2);
    }
}

/// Find a retained counter with its exact expected value.
fn has_counter(snapshot: &[SnapshotEntry], name: &str, expected: u64) -> bool {
    snapshot.iter().any(|entry| {
        entry.0.kind() == MetricKind::Counter
            && entry.0.key().name() == name
            && matches!(&entry.3, DebugValue::Counter(value) if *value == expected)
    })
}

/// Find a retained histogram containing the single expected sample.
fn has_single_sample_histogram(snapshot: &[SnapshotEntry], name: &str) -> bool {
    snapshot.iter().any(|entry| {
        entry.0.kind() == MetricKind::Histogram
            && entry.0.key().name() == name
            && matches!(&entry.3, DebugValue::Histogram(samples) if samples.len() == 1)
    })
}

/// Check that the retained snapshot contains only bounded console series.
fn assert_bounded_console_snapshot(snapshot: &[SnapshotEntry]) {
    assert_eq!(
        snapshot.len(),
        6,
        "only the six exact bounded series survive: {snapshot:?}"
    );
    for (name, expected) in [
        (BUNDLE_GENERATIONS_TOTAL, 1),
        (NINJA_BUILD_EXECUTIONS_TOTAL, 1),
        (BUNDLE_CONSOLE_EDGES_TOTAL, 2),
        (NINJA_BUILD_CONSOLE_EDGES_TOTAL, 2),
    ] {
        assert!(
            has_counter(snapshot, name, expected),
            "snapshot must retain {name} with value {expected}: {snapshot:?}"
        );
    }
    for name in [BUNDLE_GENERATION_DURATION, NINJA_BUILD_EXECUTION_DURATION] {
        assert!(
            has_single_sample_histogram(snapshot, name),
            "snapshot must retain one sample for {name}: {snapshot:?}"
        );
    }
    assert!(snapshot.iter().all(|entry| {
        entry
            .0
            .key()
            .labels()
            .all(|label| !label.value().contains("private"))
    }));
}

#[test]
fn recorder_retains_exact_bounded_console_scheduling_series() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, record_console_series);
    let snapshot = snapshotter.snapshot().into_vec();
    assert_bounded_console_snapshot(&snapshot);
}
