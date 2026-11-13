//! Tests for bounded forwarding telemetry.

use super::{
    FORWARD_FAILURES_TOTAL, ForwardFailureKind, ForwardRoute, ForwardStream,
    NINJA_STATUS_OVERSIZED_LINES_TOTAL, forward_child_output,
    forward_child_output_with_ninja_status, tests::FailingWriter, tests::record_each,
};
use metrics_util::{
    MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use std::{
    collections::BTreeMap,
    io::Cursor,
    sync::{Arc, atomic::AtomicUsize},
};

/// Record one counter increment for each line that crosses the retained-byte bound.
#[test]
fn records_oversized_status_lines_once_per_line() {
    let mut input = vec![b'x'; 513];
    input.push(b'\n');
    input.extend(std::iter::repeat_n(b'x', 513));
    input.push(b'\n');
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        forward_child_output_with_ninja_status(
            Cursor::new(input),
            std::io::sink(),
            |_current, _total, _description| {},
            ForwardRoute::new(ForwardStream::Stdout, &mut record_each),
        );
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Counter
                && entry.0.key().name() == NINJA_STATUS_OVERSIZED_LINES_TOTAL
                && entry.0.key().labels().next().is_none()
                && matches!(entry.3, DebugValue::Counter(2))
        }),
        "expected two unlabelled {NINJA_STATUS_OVERSIZED_LINES_TOTAL} increments: {snapshot:?}"
    );
}

/// Count a failing stderr destination under its bounded labels.
///
/// A destination that rejects stderr is the case the forwarding boundary exists
/// for: the child keeps running, the stream is lost, and the only record is
/// this counter and its warning. Asserting the labels here is what keeps
/// `stream="stderr"` from silently regressing to the stdout series.
#[test]
fn records_a_stderr_destination_failure() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let writes = Arc::new(AtomicUsize::new(0));

    let stats = metrics::with_local_recorder(&recorder, || {
        forward_child_output(
            Cursor::new(b"child stderr\n".to_vec()),
            FailingWriter::new(writes.clone()),
            ForwardRoute::new(ForwardStream::Stderr, &mut record_each),
        )
    });

    assert_eq!(
        stats.failure,
        Some(ForwardFailureKind::Write),
        "a rejected destination write must be classified as a write failure"
    );
    let snapshot = snapshotter.snapshot().into_vec();
    assert!(
        snapshot.iter().any(|entry| {
            let labels: BTreeMap<&str, &str> = entry
                .0
                .key()
                .labels()
                .map(|label| (label.key(), label.value()))
                .collect();
            entry.0.kind() == MetricKind::Counter
                && entry.0.key().name() == FORWARD_FAILURES_TOTAL
                && labels.len() == 2
                && labels.get("stream") == Some(&"stderr")
                && labels.get("failure_kind") == Some(&"write")
                && matches!(entry.3, DebugValue::Counter(1))
        }),
        "expected one labelled {FORWARD_FAILURES_TOTAL} increment: {snapshot:?}"
    );
}
