//! Verify bounded `netsuke check` metric registrations.

use super::*;
use netsuke::runner::{CHECK_DURATION, CHECK_TOTAL};

/// Retain only the fixed outcome label for the check counter and duration.
#[test]
fn recorder_retains_bounded_check_series() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        counter!(CHECK_TOTAL, "outcome" => "success").increment(1);
        histogram!(CHECK_DURATION, "outcome" => "success").record(0.04);
        counter!(CHECK_TOTAL, "outcome" => "unbounded").increment(1);
        histogram!(CHECK_DURATION, "outcome" => "unbounded").record(0.05);
        counter!(CHECK_TOTAL).increment(1);
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        2,
        "only the bounded check counter and duration are retained: {snapshot:?}"
    );
    assert_retained_check_counter(&snapshot);
    assert_retained_check_duration(&snapshot);
}

/// Assert that the recorder retained the successful check counter.
fn assert_retained_check_counter(snapshot: &[SnapshotEntry]) {
    assert!(
        snapshot
            .iter()
            .any(|entry| is_retained_outcome_counter(entry, CHECK_TOTAL, "success")),
        "snapshot should retain the bounded check counter: {snapshot:?}"
    );
}

/// Assert that the recorder retained the successful check duration.
fn assert_retained_check_duration(snapshot: &[SnapshotEntry]) {
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Histogram
                && entry.0.key().name() == CHECK_DURATION
                && matches!(
                    entry.0.key().labels().collect::<Vec<_>>().as_slice(),
                    [label] if label.key() == "outcome" && label.value() == "success"
                )
                && matches!(entry.3, DebugValue::Histogram(ref values) if values.as_slice() == [0.04])
        }),
        "snapshot should retain the bounded check duration: {snapshot:?}"
    );
}
