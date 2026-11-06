//! Verify forwarding failures are classified and counted with bounded labels.
//!
//! Each case drives the production forwarding helper with an injected reader
//! or writer, then reads the counter series the helper registered through a
//! local debugging recorder. The assertions compare the exact label values, so
//! an unclassified or unbounded series fails the test rather than widening the
//! vocabulary.

use super::super::{FORWARD_FAILURES_TOTAL, ForwardFailureKind, ForwardStream};
use super::{ChunkedReader, forward_child_output};
use metrics::{SharedString, Unit};
use metrics_util::{
    CompositeKey,
    debugging::{DebugValue, DebuggingRecorder},
};
use std::{
    collections::HashMap,
    io::{self, Cursor, Read, Write},
};

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

/// Reader that fails its first read once, then reports end of input.
struct FailsOnce {
    /// Error kind reported by the failing read.
    kind: io::ErrorKind,
    /// Whether the failing read has already been served.
    failed: bool,
}

impl Read for FailsOnce {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        if self.failed {
            return Ok(0);
        }
        self.failed = true;
        Err(io::Error::new(self.kind, "read failed"))
    }
}

/// Reader that returns one chunk and then fails while the forwarder drains.
struct FailsWhileDraining {
    /// Bytes delivered by the first read.
    bytes: Vec<u8>,
    /// Whether the chunk has already been delivered.
    delivered: bool,
}

impl Read for FailsWhileDraining {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.delivered {
            return Err(io::Error::other("drain read failed"));
        }
        self.delivered = true;
        let count = self.bytes.len().min(buffer.len());
        let (Some(source), Some(destination)) = (self.bytes.get(..count), buffer.get_mut(..count))
        else {
            // `count` is the smaller of the two lengths, so both slices
            // always exist; report EOF rather than panicking if that changes.
            return Ok(0);
        };
        destination.copy_from_slice(source);
        Ok(count)
    }
}

/// Reader that reports one byte more than the buffer can hold, then EOF.
#[derive(Default)]
struct OverlongOnce {
    /// Whether the invalid count has already been reported.
    reported: bool,
}

impl Read for OverlongOnce {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.reported {
            return Ok(0);
        }
        self.reported = true;
        Ok(buffer.len().saturating_add(1))
    }
}

/// Reader that reports an interruption a fixed number of times before EOF.
struct Interrupting {
    /// Interruptions still to report.
    remaining: usize,
}

impl Read for Interrupting {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        if self.remaining > 0 {
            self.remaining -= 1;
            return Err(io::Error::new(io::ErrorKind::Interrupted, "interrupted"));
        }
        Ok(0)
    }
}

/// Writer that rejects every chunk with a closed pipe.
struct ClosedWriter;

impl Write for ClosedWriter {
    fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "sink closed"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Writer that accepts its chunk but fails the following flush.
#[derive(Default)]
struct FlushFails {
    /// Number of chunk writes attempted.
    writes: usize,
    /// Bytes accepted across those writes.
    accepted: Vec<u8>,
}

impl Write for FlushFails {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        self.accepted.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "flush failed"))
    }
}

#[test]
fn read_failure_is_classified_and_counted_once() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let stats = metrics::with_local_recorder(&recorder, || {
        forward_child_output(
            FailsOnce {
                kind: io::ErrorKind::Other,
                failed: false,
            },
            io::sink(),
            ForwardStream::Stdout,
        )
    });

    assert_eq!(stats.failure, Some(ForwardFailureKind::Read));
    assert!(!stats.drain_failed);
    assert_eq!(stats.bytes_read, 0);
    assert_eq!(
        failure_series(&snapshotter.snapshot().into_vec()),
        vec![("stdout".to_owned(), "read".to_owned(), 1)]
    );
}

#[test]
fn invalid_read_count_is_classified_as_a_read_failure() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let stats = metrics::with_local_recorder(&recorder, || {
        forward_child_output(OverlongOnce::default(), io::sink(), ForwardStream::Stdout)
    });

    assert_eq!(stats.failure, Some(ForwardFailureKind::Read));
    assert!(!stats.drain_failed);
    // The impossible count is credited to `bytes_read` before the bounds check
    // rejects it, so this case asserts the classification, not the accounting.
    assert_eq!(
        failure_series(&snapshotter.snapshot().into_vec()),
        vec![("stdout".to_owned(), "read".to_owned(), 1)]
    );
}

#[test]
fn interrupted_reads_are_retried_without_recording_a_failure() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let input = b"retried".to_vec();
    let stats = metrics::with_local_recorder(&recorder, || {
        forward_child_output(
            io::Cursor::new(input.clone()).chain(Interrupting { remaining: 2 }),
            Vec::new(),
            ForwardStream::Stdout,
        )
    });

    assert_eq!(
        (
            stats.failure,
            stats.drain_failed,
            stats.bytes_read,
            stats.bytes_written,
        ),
        (None, false, input.len(), input.len()),
        "interrupted reads must be retried without being recorded"
    );
    assert!(
        failure_series(&snapshotter.snapshot().into_vec()).is_empty(),
        "retried interruptions must not be recorded as terminal failures"
    );
}

#[test]
fn write_failures_are_counted_for_each_stream_separately() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let (stdout_stats, stderr_stats) = metrics::with_local_recorder(&recorder, || {
        let stdout = forward_child_output(
            Cursor::new(b"lost".to_vec()),
            ClosedWriter,
            ForwardStream::Stdout,
        );
        let stderr = forward_child_output(
            Cursor::new(b"lost".to_vec()),
            ClosedWriter,
            ForwardStream::Stderr,
        );
        (stdout, stderr)
    });

    assert_eq!(
        (
            stdout_stats.failure,
            stdout_stats.bytes_read,
            stdout_stats.bytes_written,
        ),
        (Some(ForwardFailureKind::Write), 4, 0),
        "a closed stdout pipe must be classified as a write failure"
    );
    assert_eq!(
        (
            stderr_stats.failure,
            stderr_stats.bytes_read,
            stderr_stats.bytes_written,
        ),
        (Some(ForwardFailureKind::Write), 4, 0),
        "a closed stderr pipe must be classified as a write failure"
    );
    assert_eq!(
        failure_series(&snapshotter.snapshot().into_vec()),
        vec![
            ("stderr".to_owned(), "write".to_owned(), 1),
            ("stdout".to_owned(), "write".to_owned(), 1),
        ]
    );
}

#[test]
fn flush_failure_keeps_accepted_bytes_without_rewriting_them() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let (stats, writer) = metrics::with_local_recorder(&recorder, || {
        let mut writer = FlushFails::default();
        let stats = forward_child_output(
            ChunkedReader::new(b"firstlast".to_vec(), vec![5, 4]),
            &mut writer,
            ForwardStream::Stdout,
        );
        (stats, writer)
    });

    assert_eq!(
        (
            stats.failure,
            stats.drain_failed,
            stats.bytes_read,
            stats.bytes_written,
        ),
        (
            Some(ForwardFailureKind::Flush),
            false,
            "firstlast".len(),
            "first".len(),
        ),
        "a failed flush must keep the bytes it already accepted"
    );
    assert_eq!(
        (writer.writes, writer.accepted.as_slice()),
        (1, b"first".as_slice()),
        "accepted bytes must not be rewritten"
    );
    assert_eq!(
        failure_series(&snapshotter.snapshot().into_vec()),
        vec![("stdout".to_owned(), "flush".to_owned(), 1)]
    );
}

#[test]
fn drain_failure_is_recorded_without_replacing_the_initiating_failure() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let stats = metrics::with_local_recorder(&recorder, || {
        forward_child_output(
            FailsWhileDraining {
                bytes: b"first".to_vec(),
                delivered: false,
            },
            ClosedWriter,
            ForwardStream::Stderr,
        )
    });

    assert_eq!(
        (
            stats.failure,
            stats.drain_failed,
            stats.bytes_read,
            stats.bytes_written,
        ),
        (Some(ForwardFailureKind::Write), true, b"first".len(), 0),
        "a drain failure must not replace the initiating write failure"
    );
    assert_eq!(
        failure_series(&snapshotter.snapshot().into_vec()),
        vec![
            ("stderr".to_owned(), "drain".to_owned(), 1),
            ("stderr".to_owned(), "write".to_owned(), 1),
        ]
    );
}

#[test]
fn successful_forwarding_records_no_failure_series() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let stats = metrics::with_local_recorder(&recorder, || {
        forward_child_output(
            Cursor::new(b"forwarded".to_vec()),
            Vec::new(),
            ForwardStream::Stdout,
        )
    });

    assert_eq!(stats.failure, None);
    assert_eq!(stats.bytes_written, "forwarded".len());
    assert!(
        snapshotter
            .snapshot()
            .into_vec()
            .iter()
            .all(|entry| { entry.0.key().name() != FORWARD_FAILURES_TOTAL })
    );
}
