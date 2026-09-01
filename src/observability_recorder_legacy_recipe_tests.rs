//! Verify bounded legacy-recipe operation metric registrations.

use super::{ConfigMetricsRecorder, SnapshotEntry};
use metrics::{counter, histogram};
use metrics_util::MetricKind;
use netsuke::runner::{LEGACY_RECIPE_EXECUTION_DURATION, LEGACY_RECIPE_EXECUTIONS_TOTAL};

/// Fixed four-label layout used by legacy-recipe operation metrics.
type LegacyMetricLabels = [(&'static str, &'static str); 4];

/// Record valid and invalid legacy-recipe operation metric registrations.
fn record_legacy_recipe_operation_series() {
    counter!(
        LEGACY_RECIPE_EXECUTIONS_TOTAL,
        "operation" => "build",
        "recipe_shell" => "powershell",
        "outcome" => "success",
        "failure_category" => "none"
    )
    .increment(1);
    histogram!(
        LEGACY_RECIPE_EXECUTION_DURATION,
        "operation" => "ninja_tool",
        "recipe_shell" => "bash",
        "outcome" => "error",
        "failure_category" => "ninja_io"
    )
    .record(0.01);
    counter!(
        LEGACY_RECIPE_EXECUTIONS_TOTAL,
        "operation" => "build",
        "recipe_shell" => "powershell",
        "outcome" => "success"
    )
    .increment(1);
    counter!(
        LEGACY_RECIPE_EXECUTIONS_TOTAL,
        "operation" => "build",
        "recipe_shell" => "powershell",
        "outcome" => "success",
        "failure_category" => "none",
        "extra" => "rejected"
    )
    .increment(1);
    histogram!(
        LEGACY_RECIPE_EXECUTION_DURATION,
        "operation" => "unbounded",
        "recipe_shell" => "powershell",
        "outcome" => "success",
        "failure_category" => "none"
    )
    .record(0.02);
}

/// Retain only exact bounded legacy-recipe operation metric registrations.
#[test]
fn recorder_retains_bounded_legacy_recipe_operation_series() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, record_legacy_recipe_operation_series);

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        2,
        "only exact bounded legacy-recipe operation series are retained"
    );
    assert_legacy_recipe_metric(
        &snapshot,
        MetricKind::Counter,
        LEGACY_RECIPE_EXECUTIONS_TOTAL,
        [
            ("operation", "build"),
            ("recipe_shell", "powershell"),
            ("outcome", "success"),
            ("failure_category", "none"),
        ],
    );
    assert_legacy_recipe_metric(
        &snapshot,
        MetricKind::Histogram,
        LEGACY_RECIPE_EXECUTION_DURATION,
        [
            ("operation", "ninja_tool"),
            ("recipe_shell", "bash"),
            ("outcome", "error"),
            ("failure_category", "ninja_io"),
        ],
    );
}

/// Assert that one retained legacy-recipe metric has exactly `labels`.
fn assert_legacy_recipe_metric(
    snapshot: &[SnapshotEntry],
    kind: MetricKind,
    metric_name: &str,
    labels: LegacyMetricLabels,
) {
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == kind
                && entry.0.key().name() == metric_name
                && entry.0.key().labels().count() == labels.len()
                && labels.iter().all(|(key, value)| {
                    entry
                        .0
                        .key()
                        .labels()
                        .any(|label| label.key() == *key && label.value() == *value)
                })
        }),
        "expected retained {metric_name} with bounded labels {labels:?}: {snapshot:?}"
    );
}
