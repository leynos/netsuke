//! Unit tests for streaming child process output forwarding.
//!
//! JSON output and `progress = never` use `forward_child_output` directly, so
//! these parser checks cover only the progress-enabled stdout path.

use super::{
    NinjaStatusLineState, NinjaStatusParsingReader, NinjaTaskProgressTracker, forward_child_output,
    forward_child_output_with_ninja_status,
};
use proptest::prelude::*;
use std::{
    io::{self, BufReader, Cursor, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Clone)]
struct FailingWriter {
    writes: Arc<AtomicUsize>,
}

impl FailingWriter {
    fn new(writes: Arc<AtomicUsize>) -> Self {
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

/// Generate a large stream without retaining its forwarded bytes in memory.
#[cfg(unix)]
struct RepeatingByteReader {
    /// Bytes still available to read.
    remaining: usize,
}

#[cfg(unix)]
impl RepeatingByteReader {
    /// Construct a reader producing `byte_count` bytes without newlines.
    const fn new(byte_count: usize) -> Self {
        Self {
            remaining: byte_count,
        }
    }
}

#[cfg(unix)]
impl Read for RepeatingByteReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let count = self.remaining.min(buf.len());
        let Some(destination) = buf.get_mut(..count) else {
            return Err(io::Error::other(
                "repeating reader destination range is invalid",
            ));
        };
        destination.fill(b'x');
        self.remaining = self.remaining.saturating_sub(count);
        Ok(count)
    }
}

/// Return the process's current peak resident-set size in bytes.
#[cfg(unix)]
fn peak_rss_bytes() -> io::Result<u64> {
    let usage = nix::sys::resource::getrusage(nix::sys::resource::UsageWho::RUSAGE_SELF)
        .map_err(io::Error::other)?;
    let resident_set = u64::try_from(usage.max_rss())
        .map_err(|_| io::Error::other("getrusage returned a negative RSS"))?;
    #[cfg(target_os = "macos")]
    let bytes = resident_set;
    #[cfg(not(target_os = "macos"))]
    let bytes = resident_set.saturating_mul(1024);
    Ok(bytes)
}

#[test]
fn forward_output_writes_all_bytes_when_parent_alive() {
    let input = b"alpha\nbravo\ncharlie\n".to_vec();
    let reader = BufReader::new(Cursor::new(input.clone()));
    let stats = forward_child_output(reader, Vec::new(), "stdout");

    assert_eq!(stats.bytes_read, input.len());
    assert_eq!(stats.bytes_written, input.len());
    assert!(!stats.write_failed);
}

#[test]
fn forward_output_continues_draining_after_write_failure() {
    let input = b"echo-one\necho-two\necho-three\n".to_vec();
    let reader = BufReader::new(Cursor::new(input.clone()));
    let write_attempts = Arc::new(AtomicUsize::new(0));
    let failing_writer = FailingWriter::new(write_attempts.clone());
    let stats = forward_child_output(reader, failing_writer, "stdout");

    assert_eq!(stats.bytes_read, input.len());
    assert_eq!(write_attempts.load(Ordering::SeqCst), 1);
    assert!(stats.write_failed);
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
        "stdout",
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
        "stdout",
    );

    assert_eq!(forwarded, input);
    assert_eq!(stats.bytes_read, input.len());
    assert_eq!(stats.bytes_written, input.len());
}

#[test]
fn oversized_line_forwards_unchanged_and_next_status_line_updates_progress() {
    let mut input = vec![b'x'; TEST_MAX_LINE_BYTES + 1];
    input.extend_from_slice(b"\n[2/3] cc -c resumed.c\n");
    let mut output = Vec::new();
    let mut updates = Vec::new();

    let stats = forward_child_output_with_ninja_status(
        Cursor::new(input.clone()),
        &mut output,
        |current, total, description| updates.push((current, total, description.to_owned())),
        "stdout",
    );

    assert_eq!(stats.bytes_read, input.len());
    assert_eq!(stats.bytes_written, input.len());
    assert_eq!(output, input);
    assert_eq!(updates, vec![(2, 3, "cc -c resumed.c".to_owned())]);
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
        "stdout",
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

#[cfg(unix)]
#[test]
#[ignore = "getrusage is process-global and varies with allocator and OS accounting"]
fn status_parsing_peak_rss_does_not_scale_with_a_large_unterminated_stream() {
    const STREAM_BYTES: usize = 256 * 1024 * 1024;
    const RSS_ALLOWANCE_BYTES: u64 = 4 * 1024 * 1024;

    let plain_before = peak_rss_bytes().expect("getrusage should report initial RSS");
    let plain_stats =
        forward_child_output(RepeatingByteReader::new(STREAM_BYTES), io::sink(), "stdout");
    let plain_after = peak_rss_bytes().expect("getrusage should report plain forwarding RSS");
    let status_before = peak_rss_bytes().expect("getrusage should report status forwarding RSS");
    let status_stats = forward_child_output_with_ninja_status(
        RepeatingByteReader::new(STREAM_BYTES),
        io::sink(),
        |_current, _total, _description| {},
        "stdout",
    );
    let status_after = peak_rss_bytes().expect("getrusage should report status parsing RSS");

    assert_eq!(plain_stats.bytes_read, STREAM_BYTES);
    assert_eq!(status_stats.bytes_read, STREAM_BYTES);
    assert!(plain_after >= plain_before);
    assert!(status_after >= status_before);
    assert!(
        status_after.saturating_sub(status_before) <= RSS_ALLOWANCE_BYTES,
        "status parsing should add only a fixed RSS allowance, not one proportional to {STREAM_BYTES} bytes"
    );
}
