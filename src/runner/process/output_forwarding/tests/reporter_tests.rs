//! Verify the production reporter owns the forwarding-failure counter.
//!
//! These cases pass `report_forward_failure` — the reporter production
//! installs — into the forwarding helpers and read the counter back through a
//! local debugging recorder. A copy loop that counted telemetry itself, or a
//! boundary that stopped calling `record_failure`, fails here.

use super::super::report_forward_failure;
use super::support::{FailsOnce, FailsWhileDraining, RejectingWriter};
use crate::runner::process::streaming::{
    FORWARD_FAILURES_TOTAL, ForwardFailureKind, ForwardRoute, ForwardStream, forward_child_output,
};
use metrics::{SharedString, Unit};
use metrics_util::{
    CompositeKey,
    debugging::{DebugValue, DebuggingRecorder},
};
use std::{collections::HashMap, io, io::Cursor};

/// One entry of a drained debugging-recorder snapshot.
type SnapshotEntry = (CompositeKey, Option<Unit>, Option<SharedString>, DebugValue);

/// Collect the recorded forwarding-failure series as `(stream, kind, count)`.
fn failure_series(entries: &[SnapshotEntry]) -> Vec<(String, String, u64)> {
    let mut series: Vec<_> = entries
        .iter()
        .filter(|entry| entry.0.key().name() == FORWARD_FAILURES_TOTAL)
        .map(|entry| {
            let labels: HashMap<_, _> = entry
                .0
                .key()
                .labels()
                .map(|label| (label.key(), label.value().to_owned()))
                .collect();
            let DebugValue::Counter(count) = &entry.3 else {
                panic!("forwarding-failure series must be a counter: {entry:?}");
            };
            let stream = labels
                .get("stream")
                .cloned()
                .expect("recorded forwarding-failure series must carry a stream label");
            let kind = labels
                .get("failure_kind")
                .cloned()
                .expect("recorded forwarding-failure series must carry a failure_kind label");
            (stream, kind, *count)
        })
        .collect();
    series.sort();
    series
}

#[test]
fn a_write_failure_reaches_the_production_counter() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let stats = metrics::with_local_recorder(&recorder, || {
        let mut report = report_forward_failure;
        forward_child_output(
            Cursor::new(b"lost".to_vec()),
            RejectingWriter,
            ForwardRoute::new(ForwardStream::Stdout, &mut report),
        )
    });

    assert_eq!(stats.failure, Some(ForwardFailureKind::Write));
    assert_eq!(
        failure_series(&snapshotter.snapshot().into_vec()),
        vec![("stdout".to_owned(), "write".to_owned(), 1)],
        "the production reporter must count the write failure exactly once"
    );
}

#[test]
fn a_read_failure_reaches_the_production_counter() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let stats = metrics::with_local_recorder(&recorder, || {
        let mut report = report_forward_failure;
        forward_child_output(
            FailsOnce::new(),
            io::sink(),
            ForwardRoute::new(ForwardStream::Stdout, &mut report),
        )
    });

    assert_eq!(stats.failure, Some(ForwardFailureKind::Read));
    assert_eq!(
        failure_series(&snapshotter.snapshot().into_vec()),
        vec![("stdout".to_owned(), "read".to_owned(), 1)],
        "the production reporter must count the read failure exactly once"
    );
}

#[test]
fn a_write_failure_and_its_drain_failure_both_reach_the_production_counter() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let stats = metrics::with_local_recorder(&recorder, || {
        let mut report = report_forward_failure;
        forward_child_output(
            FailsWhileDraining::new(b"first".to_vec()),
            RejectingWriter,
            ForwardRoute::new(ForwardStream::Stderr, &mut report),
        )
    });

    assert_eq!(stats.failure, Some(ForwardFailureKind::Write));
    assert!(stats.drain_failed, "the drain must fail in this case");
    assert_eq!(
        failure_series(&snapshotter.snapshot().into_vec()),
        vec![
            ("stderr".to_owned(), "drain".to_owned(), 1),
            ("stderr".to_owned(), "write".to_owned(), 1),
        ],
        "the production reporter must count the write and drain failures separately"
    );
}
