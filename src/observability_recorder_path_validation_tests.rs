//! Verify bounded CLI path-validation metric registrations.

use super::*;

/// Retain only the fixed source and reason labels for CLI path-validation counters.
#[test]
fn recorder_retains_bounded_cli_path_validation_series() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        counter!(PATH_VALIDATION_TOTAL, "source" => "file", "reason" => "non_utf8").increment(1);
        counter!(PATH_VALIDATION_TOTAL, "source" => "directory", "reason" => "non_utf8")
            .increment(1);
        counter!(PATH_VALIDATION_TOTAL, "source" => "unbounded", "reason" => "non_utf8")
            .increment(1);
        counter!(PATH_VALIDATION_TOTAL, "source" => "file", "reason" => "unbounded").increment(1);
        counter!(PATH_VALIDATION_TOTAL, "source" => "file").increment(1);
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        2,
        "only the two bounded CLI path-validation series are retained"
    );
    for source in ["file", "directory"] {
        assert_path_validation_counter(&snapshot, source);
    }
}

/// Verify one retained CLI path-validation counter with the specified source and the `non_utf8` reason.
fn assert_path_validation_counter(snapshot: &[SnapshotEntry], source: &str) {
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Counter
                && entry.0.key().name() == PATH_VALIDATION_TOTAL
                && entry.0.key().labels().count() == 2
                && entry
                    .0
                    .key()
                    .labels()
                    .any(|label| label.key() == "source" && label.value() == source)
                && entry
                    .0
                    .key()
                    .labels()
                    .any(|label| label.key() == "reason" && label.value() == "non_utf8")
                && matches!(entry.3, DebugValue::Counter(1))
        }),
        "recorder should retain the {source:?} path-validation series: {snapshot:?}"
    );
}
