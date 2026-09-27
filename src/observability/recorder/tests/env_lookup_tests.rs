//! Verify the bounded manifest environment-lookup counter series.

use super::{ConfigMetricsRecorder, SnapshotEntry};
use metrics::counter;
use metrics_util::{MetricKind, debugging::DebugValue};
use netsuke::manifest::ENV_LOOKUP_TOTAL;

/// Retain only the closed `outcome` vocabulary of environment-lookup counters.
#[test]
fn recorder_retains_bounded_env_lookup_series() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        counter!(ENV_LOOKUP_TOTAL, "outcome" => "success").increment(1);
        counter!(ENV_LOOKUP_TOTAL, "outcome" => "blocked").increment(2);
        counter!(ENV_LOOKUP_TOTAL, "outcome" => "not_present").increment(3);
        counter!(ENV_LOOKUP_TOTAL, "outcome" => "not_unicode").increment(4);
        counter!(ENV_LOOKUP_TOTAL, "outcome" => "unbounded").increment(1);
        counter!(ENV_LOOKUP_TOTAL, "outcome" => "success", "name" => "PATH").increment(1);
        counter!(ENV_LOOKUP_TOTAL).increment(1);
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        4,
        "only the four bounded environment-lookup series are retained"
    );
    assert_env_lookup_counter(&snapshot, "success", 1);
    assert_env_lookup_counter(&snapshot, "blocked", 2);
    assert_env_lookup_counter(&snapshot, "not_present", 3);
    assert_env_lookup_counter(&snapshot, "not_unicode", 4);
}

/// Assert that one retained environment-lookup counter carries the sole
/// `outcome` label and the expected count.
fn assert_env_lookup_counter(snapshot: &[SnapshotEntry], outcome: &str, expected: u64) {
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Counter
                && entry.0.key().name() == ENV_LOOKUP_TOTAL
                && entry.0.key().labels().count() == 1
                && entry
                    .0
                    .key()
                    .labels()
                    .any(|label| label.key() == "outcome" && label.value() == outcome)
                && matches!(entry.3, DebugValue::Counter(observed) if observed == expected)
        }),
        "expected retained environment-lookup counter for {outcome}={expected}: {snapshot:?}"
    );
}
