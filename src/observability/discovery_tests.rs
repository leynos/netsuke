//! Regression coverage for bounded configuration-discovery telemetry.

use super::*;
use metrics::{SharedString, Unit};
use metrics_util::{CompositeKey, MetricKind, debugging::DebugValue};
use netsuke::cli::{DISCOVERY_DURATION, DISCOVERY_TOTAL};

type SnapshotEntry = (CompositeKey, Option<Unit>, Option<SharedString>, DebugValue);

/// The production recorder admits the discovery series emitted at the
/// composition boundary, while rejecting unbounded variants.
#[test]
fn recorder_retains_discovery_series() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        counter!(DISCOVERY_TOTAL, "outcome" => "success").increment(1);
        counter!(DISCOVERY_TOTAL, "outcome" => "error").increment(1);
        histogram!(DISCOVERY_DURATION).record(0.01);
        // Rejected: a discovery counter carrying an extra unadmitted label.
        counter!(DISCOVERY_TOTAL, "outcome" => "success", "phase" => "diag_mode").increment(1);
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        3,
        "discovery counter (success/error) and duration histogram should be retained"
    );
    assert_discovery_counter(&snapshot, "success");
    assert_discovery_counter(&snapshot, "error");
    assert_discovery_duration(&snapshot);
}

fn assert_discovery_counter(snapshot: &[SnapshotEntry], expected_outcome: &str) {
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Counter
                && entry.0.key().name() == DISCOVERY_TOTAL
                && entry
                    .0
                    .key()
                    .labels()
                    .any(|label| label.key() == "outcome" && label.value() == expected_outcome)
                && matches!(entry.3, DebugValue::Counter(1))
        }),
        "expected retained discovery counter with outcome {expected_outcome}",
    );
}

fn assert_discovery_duration(snapshot: &[SnapshotEntry]) {
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Histogram
                && entry.0.key().name() == DISCOVERY_DURATION
                && entry.0.key().labels().count() == 0
                && matches!(entry.3, DebugValue::Histogram(ref values) if values.len() == 1)
        }),
        "expected retained discovery duration histogram with one sample",
    );
}
