//! Verify the unlabelled oversized Ninja status counter series.

use super::ConfigMetricsRecorder;
use metrics::counter;
use metrics_util::{MetricKind, debugging::DebugValue};
use netsuke::runner::NINJA_STATUS_OVERSIZED_LINES_TOTAL;

/// Retain only the unlabelled oversized Ninja status counter series.
#[test]
fn recorder_retains_unlabelled_oversized_ninja_status_counter() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        counter!(NINJA_STATUS_OVERSIZED_LINES_TOTAL).increment(1);
        counter!(NINJA_STATUS_OVERSIZED_LINES_TOTAL, "operation" => "build").increment(1);
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        1,
        "only the unlabelled oversized Ninja status counter should be retained"
    );
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Counter
                && entry.0.key().name() == NINJA_STATUS_OVERSIZED_LINES_TOTAL
                && entry.0.key().labels().next().is_none()
                && matches!(entry.3, DebugValue::Counter(1))
        }),
        "expected unlabelled {NINJA_STATUS_OVERSIZED_LINES_TOTAL} counter: {snapshot:?}"
    );
}
