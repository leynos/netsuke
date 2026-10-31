//! Verify the bounded child-output forwarding failure counter series.
//!
//! The forwarding helper records `stream` and `failure_kind` from closed
//! vocabularies, so a series the recorder rejects is silently dropped: the
//! counter handle becomes a noop and no other test notices. These cases drive
//! the real filter through the production recorder, then assert both the
//! admitted combinations and the rejected widening attempts.

use super::{ConfigMetricsRecorder, SnapshotEntry};
use metrics::counter;
use metrics_util::{MetricKind, debugging::DebugValue};
use netsuke::runner::{FORWARD_FAILURE_LABELS, FORWARD_FAILURES_TOTAL};

/// Return the retained total for one exact `(stream, failure_kind)` series.
fn retained_count(snapshot: &[SnapshotEntry], stream: &str, failure_kind: &str) -> u64 {
    snapshot
        .iter()
        .find_map(|entry| {
            if entry.0.kind() != MetricKind::Counter
                || entry.0.key().name() != FORWARD_FAILURES_TOTAL
            {
                return None;
            }
            let labels: Vec<_> = entry.0.key().labels().collect();
            let is_exact = labels.len() == 2
                && labels
                    .iter()
                    .any(|label| label.key() == "stream" && label.value() == stream)
                && labels
                    .iter()
                    .any(|label| label.key() == "failure_kind" && label.value() == failure_kind);
            match (is_exact, &entry.3) {
                (true, DebugValue::Counter(count)) => Some(*count),
                _ => None,
            }
        })
        .unwrap_or(0)
}

/// Retain every combination the forwarding helper can emit.
#[test]
fn recorder_retains_every_bounded_forwarding_failure_series() {
    let [stream_label, failure_kind_label] = FORWARD_FAILURE_LABELS.map(|(label, _)| label);
    let [streams, failure_kinds] = FORWARD_FAILURE_LABELS.map(|(_, values)| values);

    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        for stream in streams {
            for failure_kind in failure_kinds {
                counter!(
                    FORWARD_FAILURES_TOTAL,
                    stream_label => *stream,
                    failure_kind_label => *failure_kind,
                )
                .increment(1);
            }
        }
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        streams.len() * failure_kinds.len(),
        "every closed stream and failure-kind pair should be retained: {snapshot:?}"
    );
    for stream in streams {
        for failure_kind in failure_kinds {
            assert_eq!(
                retained_count(&snapshot, stream, failure_kind),
                1,
                "missing bounded series ({stream}, {failure_kind}): {snapshot:?}"
            );
        }
    }
}

/// Reject series that widen either label vocabulary or add a label.
#[test]
fn recorder_rejects_widened_forwarding_failure_series() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        counter!(
            FORWARD_FAILURES_TOTAL,
            "stream" => "private-output",
            "failure_kind" => "read",
        )
        .increment(1);
        counter!(
            FORWARD_FAILURES_TOTAL,
            "stream" => "stdout",
            "failure_kind" => "private-output",
        )
        .increment(1);
        counter!(
            FORWARD_FAILURES_TOTAL,
            "stream" => "stdout",
            "failure_kind" => "read",
            "target" => "private-target",
        )
        .increment(1);
        counter!(FORWARD_FAILURES_TOTAL, "stream" => "stdout").increment(1);
        counter!(FORWARD_FAILURES_TOTAL).increment(1);
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert!(
        snapshot.is_empty(),
        "unbounded forwarding-failure series must not be retained: {snapshot:?}"
    );
}
