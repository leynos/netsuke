//! Verify the bounded stdlib file-read counter series.

use super::{ConfigMetricsRecorder, SnapshotEntry};
use metrics::counter;
use metrics_util::{MetricKind, debugging::DebugValue};
use netsuke::stdlib::FILE_READ_TOTAL;

/// Retain only the closed `filter` and `outcome` vocabulary of file-read counters.
#[test]
fn recorder_retains_bounded_file_read_series() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        counter!(FILE_READ_TOTAL, "filter" => "contents", "outcome" => "ok").increment(1);
        counter!(FILE_READ_TOTAL, "filter" => "digest", "outcome" => "rejected").increment(1);
        counter!(FILE_READ_TOTAL, "filter" => "unbounded", "outcome" => "ok").increment(1);
        counter!(FILE_READ_TOTAL, "filter" => "contents", "outcome" => "unbounded").increment(1);
        counter!(FILE_READ_TOTAL, "filter" => "contents").increment(1);
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        2,
        "only the two bounded file-read series are retained"
    );
    assert_file_read_counter(&snapshot, "contents", "ok");
    assert_file_read_counter(&snapshot, "digest", "rejected");
}

/// Assert that one retained file-read counter carries exactly the `filter` and
/// `outcome` labels it was recorded with.
fn assert_file_read_counter(snapshot: &[SnapshotEntry], filter: &str, outcome: &str) {
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Counter
                && entry.0.key().name() == FILE_READ_TOTAL
                && entry.0.key().labels().count() == 2
                && entry
                    .0
                    .key()
                    .labels()
                    .any(|label| label.key() == "filter" && label.value() == filter)
                && entry
                    .0
                    .key()
                    .labels()
                    .any(|label| label.key() == "outcome" && label.value() == outcome)
                && matches!(entry.3, DebugValue::Counter(1))
        }),
        "expected retained file-read counter for {filter}/{outcome}: {snapshot:?}"
    );
}
