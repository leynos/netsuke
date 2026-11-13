//! Unit tests for streaming child process output forwarding.
//!
//! JSON output and `progress = never` use `forward_child_output` directly, so
//! these parser checks cover only the progress-enabled stdout path.

use super::{
    ForwardFailure, ForwardFailureKind, ForwardRoute, ForwardStream, NinjaStatusLineState,
    NinjaStatusParsingReader, NinjaTaskProgressTracker, forward_child_output,
    forward_child_output_with_ninja_status, record_failure,
};
use proptest::prelude::*;
use std::{
    io::{self, BufReader, Cursor, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

/// Reporter that records each classified failure in the shared counter.
///
/// The forwarding helpers take their reporter as a parameter, so these cases
/// pass the production reporter deliberately: they exercise the same metric
/// path production uses, through an explicit dependency rather than a hidden
/// global call inside the copy loop.
pub(super) fn record_each(stream: ForwardStream, failure: &ForwardFailure) {
    record_failure(stream, failure.kind);
}

#[path = "tests/failure_tests.rs"]
mod failure_tests;
#[path = "tests/flush_tests.rs"]
mod flush_tests;
#[path = "tests/reporter_tests.rs"]
mod reporter_tests;
#[cfg(unix)]
#[path = "tests/rss_tests.rs"]
mod rss_tests;

#[derive(Clone)]
pub(super) struct FailingWriter {
    writes: Arc<AtomicUsize>,
}

impl FailingWriter {
    pub(super) fn new(writes: Arc<AtomicUsize>) -> Self {
        Self { writes }
    }
}

impl Write for FailingWriter {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        let previous = self.writes.fetch_add(1, Ordering::SeqCst);
        let error_kind = if previous == 0 {
            io::ErrorKind::BrokenPipe
        } else {
            io::ErrorKind::Other
        };
        Err(io::Error::new(error_kind, "sink closed"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Read one stored byte stream through caller-selected chunk boundaries.
struct ChunkedReader {
    /// Bytes emitted by this reader.
    input: Vec<u8>,
    /// Requested maximum length for successive reads.
    chunk_sizes: Vec<usize>,
    /// Offset of the next emitted byte.
    offset: usize,
    /// Index of the next requested chunk length.
    chunk_index: usize,
}

impl ChunkedReader {
    /// Construct a reader that exposes `input` at the supplied boundaries.
    fn new(input: Vec<u8>, chunk_sizes: Vec<usize>) -> Self {
        Self {
            input,
            chunk_sizes,
            offset: 0,
            chunk_index: 0,
        }
    }
}

impl Read for ChunkedReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let remaining = self.input.len().saturating_sub(self.offset);
        if remaining == 0 || buf.is_empty() {
            return Ok(0);
        }
        let requested = self
            .chunk_sizes
            .get(self.chunk_index)
            .copied()
            .unwrap_or(buf.len());
        self.chunk_index = self.chunk_index.saturating_add(1);
        let count = remaining.min(requested.max(1)).min(buf.len());
        let end = self.offset.saturating_add(count);
        let Some(source) = self.input.get(self.offset..end) else {
            return Err(io::Error::other("chunk reader source range is invalid"));
        };
        let Some(destination) = buf.get_mut(..count) else {
            return Err(io::Error::other(
                "chunk reader destination range is invalid",
            ));
        };
        destination.copy_from_slice(source);
        self.offset = end;
        Ok(count)
    }
}

type TestStatusObserver = fn(u32, u32, &str);
const TEST_MAX_LINE_BYTES: usize =
    NinjaStatusParsingReader::<ChunkedReader, TestStatusObserver>::MAX_LINE_BYTES;

/// Assert status-aware forwarding preserves bytes and accepted progress updates.
fn assert_ninja_status_forwarding<R>(reader: R, input: &[u8], expected_updates: &[(u32, u32, &str)])
where
    R: Read,
{
    let mut output = Vec::new();
    let mut updates = Vec::new();

    let stats = forward_child_output_with_ninja_status(
        reader,
        &mut output,
        |current, total, description| updates.push((current, total, description.to_owned())),
        ForwardRoute::new(ForwardStream::Stdout, &mut record_each),
    );

    assert_eq!(stats.bytes_read, input.len());
    assert_eq!(stats.bytes_written, input.len());
    assert_eq!(output, input);
    let expected_update_strings = expected_updates
        .iter()
        .map(|(current, total, description)| (*current, *total, (*description).to_owned()))
        .collect::<Vec<_>>();
    assert_eq!(updates, expected_update_strings);
}

#[test]
fn forward_output_writes_all_bytes_when_parent_alive() {
    let input = b"alpha\nbravo\ncharlie\n".to_vec();
    let reader = BufReader::new(Cursor::new(input.clone()));
    let stats = forward_child_output(
        reader,
        Vec::new(),
        ForwardRoute::new(ForwardStream::Stdout, &mut record_each),
    );

    assert_eq!(stats.bytes_read, input.len());
    assert_eq!(stats.bytes_written, input.len());
    assert_eq!(stats.failure, None);
}

#[test]
fn forward_output_continues_draining_after_write_failure() {
    let input = b"echo-one\necho-two\necho-three\n".to_vec();
    let reader = BufReader::new(Cursor::new(input.clone()));
    let write_attempts = Arc::new(AtomicUsize::new(0));
    let failing_writer = FailingWriter::new(write_attempts.clone());
    let stats = forward_child_output(
        reader,
        failing_writer,
        ForwardRoute::new(ForwardStream::Stdout, &mut record_each),
    );

    assert_eq!(stats.bytes_read, input.len());
    assert_eq!(write_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(stats.failure, Some(ForwardFailureKind::Write));
    assert_eq!(stats.bytes_written, 0);
}

#[test]
fn forward_output_with_ninja_status_parses_monotonic_updates() {
    let input = concat!(
        "[1/3] cc -c a.c\n",
        "warning: not a status line\n",
        "[2/3] cc -c b.c\n",
        "[1/3] stale line\n",
        "[3/3] cc -c c.c\n",
    );
    let reader = BufReader::new(Cursor::new(input.as_bytes().to_vec()));
    let mut updates = Vec::new();
    let stats = forward_child_output_with_ninja_status(
        reader,
        Vec::new(),
        |current, total, description| {
            updates.push((current, total, description.to_owned()));
        },
        ForwardRoute::new(ForwardStream::Stdout, &mut record_each),
    );

    assert_eq!(stats.bytes_read, input.len());
    assert_eq!(stats.bytes_written, input.len());
    assert_eq!(
        updates,
        vec![
            (1, 3, "cc -c a.c".to_owned()),
            (2, 3, "cc -c b.c".to_owned()),
            (3, 3, "cc -c c.c".to_owned()),
        ]
    );
}

#[test]
fn bounded_status_reader_forwards_large_unterminated_output() {
    let input = vec![b'x'; 256 * 1024];
    {
        let mut empty = io::empty();
        let mut observer = |_current, _total, _description: &str| {};
        let mut parser = NinjaStatusParsingReader {
            inner: &mut empty,
            tracker: NinjaTaskProgressTracker::default(),
            pending_line: Vec::with_capacity(TEST_MAX_LINE_BYTES),
            line_state: NinjaStatusLineState::Buffering,
            observer: &mut observer,
        };
        for chunk in input.chunks(127) {
            parser.consume_bytes(chunk);
            assert!(
                parser.pending_line_len() <= TEST_MAX_LINE_BYTES,
                "status parsing must not retain an unbounded partial line"
            );
        }
        assert_eq!(parser.pending_line_len(), 0);
    }
    let mut forwarded = Vec::new();
    let stats = forward_child_output_with_ninja_status(
        ChunkedReader::new(input.clone(), vec![127; 4096]),
        &mut forwarded,
        |_current, _total, _description| {},
        ForwardRoute::new(ForwardStream::Stdout, &mut record_each),
    );

    assert_eq!(forwarded, input);
    assert_eq!(stats.bytes_read, input.len());
    assert_eq!(stats.bytes_written, input.len());
}

#[test]
fn oversized_line_forwards_unchanged_and_next_status_line_updates_progress() {
    let mut input = vec![b'x'; TEST_MAX_LINE_BYTES + 1];
    input.extend_from_slice(b"\n[2/3] cc -c resumed.c\n");

    assert_ninja_status_forwarding(
        Cursor::new(input.clone()),
        &input,
        &[(2, 3, "cc -c resumed.c")],
    );
}

#[test]
fn oversized_status_suffix_stays_ignored_across_read_boundaries() {
    let mut input = vec![b'x'; TEST_MAX_LINE_BYTES];
    input.extend_from_slice(b"[1/2] spoofed.c\n[2/2] cc -c resumed.c\n");
    let reader = ChunkedReader::new(input.clone(), vec![TEST_MAX_LINE_BYTES, 4, 11, 1, 5, 7, 64]);

    assert_ninja_status_forwarding(reader, &input, &[(2, 2, "cc -c resumed.c")]);
}

#[test]
fn split_status_line_updates_once_and_eof_partial_line_stays_bounded() {
    let input = b"[1/2] cc -c split.c\n[2/2] cc -c eof.c".to_vec();
    let reader = ChunkedReader::new(input.clone(), vec![5, 3, 7, 2, 11, 1, 9]);
    let mut output = Vec::new();
    let mut updates = Vec::new();

    let stats = forward_child_output_with_ninja_status(
        reader,
        &mut output,
        |current, total, description| updates.push((current, total, description.to_owned())),
        ForwardRoute::new(ForwardStream::Stdout, &mut record_each),
    );

    assert_eq!(stats.bytes_read, input.len());
    assert_eq!(output, input);
    assert_eq!(
        updates,
        vec![
            (1, 2, "cc -c split.c".to_owned()),
            (2, 2, "cc -c eof.c".to_owned()),
        ]
    );
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, .. ProptestConfig::default() })]

    #[test]
    fn status_reader_bounds_retained_bytes_and_forwards_arbitrary_input(
        input in prop::collection::vec(any::<u8>(), 0..8192),
        chunk_sizes in prop::collection::vec(1_usize..65, 0..128),
    ) {
        let mut reader = ChunkedReader::new(input.clone(), chunk_sizes);
        let mut observer = |_current, _total, _description: &str| {};
        let mut parser = NinjaStatusParsingReader {
            inner: &mut reader,
            tracker: NinjaTaskProgressTracker::default(),
            pending_line: Vec::with_capacity(TEST_MAX_LINE_BYTES),
            line_state: NinjaStatusLineState::Buffering,
            observer: &mut observer,
        };
        let mut forwarded = Vec::new();
        let mut buffer = [0_u8; 128];

        loop {
            let count = parser
                .read(&mut buffer)
                .expect("generated reader should remain healthy");
            if count == 0 {
                break;
            }
            let written = buffer
                .get(..count)
                .expect("reported read count should fit the supplied buffer");
            forwarded
                .write_all(written)
                .expect("vector writer should accept forwarded bytes");
            prop_assert!(parser.pending_line_len() <= TEST_MAX_LINE_BYTES);
        }

        prop_assert_eq!(forwarded, input);
    }
}
