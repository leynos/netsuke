//! Tests for bounded oversized Ninja status-line telemetry.

use super::{NINJA_STATUS_OVERSIZED_LINES_TOTAL, forward_child_output_with_ninja_status};
use metrics_util::{
    MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use std::io::Cursor;

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
            "stdout",
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
