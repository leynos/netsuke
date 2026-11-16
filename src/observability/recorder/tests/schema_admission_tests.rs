//! Verify the bounded manifest schema-admission counter series.

use super::{ConfigMetricsRecorder, SnapshotEntry};
use metrics::counter;
use metrics_util::{MetricKind, debugging::DebugValue};
use netsuke::manifest::SCHEMA_ADMISSION_TOTAL;

/// Retain only the closed `outcome` vocabulary of schema-admission counters.
#[test]
fn recorder_retains_bounded_schema_admission_series() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        counter!(SCHEMA_ADMISSION_TOTAL, "outcome" => "admitted").increment(1);
        counter!(SCHEMA_ADMISSION_TOTAL, "outcome" => "missing").increment(2);
        counter!(SCHEMA_ADMISSION_TOTAL, "outcome" => "not_string").increment(3);
        counter!(SCHEMA_ADMISSION_TOTAL, "outcome" => "invalid").increment(4);
        counter!(SCHEMA_ADMISSION_TOTAL, "outcome" => "unsupported").increment(5);
        counter!(SCHEMA_ADMISSION_TOTAL, "outcome" => "not_canonical").increment(6);
        // Rejected: a value outside the closed vocabulary, an extra label, and a
        // series with no label at all.
        counter!(SCHEMA_ADMISSION_TOTAL, "outcome" => "1.1.0").increment(1);
        counter!(SCHEMA_ADMISSION_TOTAL, "outcome" => "admitted", "path" => "Netsukefile")
            .increment(1);
        counter!(SCHEMA_ADMISSION_TOTAL).increment(1);
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        6,
        "only the six bounded schema-admission series are retained"
    );
    assert_schema_admission_counter(&snapshot, "admitted", 1);
    assert_schema_admission_counter(&snapshot, "missing", 2);
    assert_schema_admission_counter(&snapshot, "not_string", 3);
    assert_schema_admission_counter(&snapshot, "invalid", 4);
    assert_schema_admission_counter(&snapshot, "unsupported", 5);
    assert_schema_admission_counter(&snapshot, "not_canonical", 6);
}

/// Assert that one retained schema-admission counter carries the sole
/// `outcome` label and the expected count.
fn assert_schema_admission_counter(snapshot: &[SnapshotEntry], outcome: &str, expected: u64) {
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Counter
                && entry.0.key().name() == SCHEMA_ADMISSION_TOTAL
                && entry.0.key().labels().count() == 1
                && entry
                    .0
                    .key()
                    .labels()
                    .any(|label| label.key() == "outcome" && label.value() == outcome)
                && matches!(entry.3, DebugValue::Counter(observed) if observed == expected)
        }),
        "expected retained schema-admission counter for {outcome}={expected}: {snapshot:?}"
    );
}
