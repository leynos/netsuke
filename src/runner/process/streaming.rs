//! Streaming helpers for subprocess output forwarding.

use super::ninja_status::{NinjaTaskProgressTracker, parse_ninja_status_line};
use std::io::{self, Read, Write};

/// Forwarding statistics for a child output stream.
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub(super) struct ForwardStats {
    /// Bytes consumed from the child stream.
    pub(super) bytes_read: usize,
    /// Bytes accepted by the forwarding writer.
    pub(super) bytes_written: usize,
    /// Whether the forwarding writer failed, truncating output.
    pub(super) write_failed: bool,
}

/// Read wrapper that counts bytes surfaced to the caller.
struct CountingReader<'a, R> {
    /// Wrapped reader.
    inner: &'a mut R,
    /// Saturating count of bytes read so far.
    read: u64,
}

impl<R: Read> Read for CountingReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buf)?;
        self.read = self.read.saturating_add(count as u64);
        Ok(count)
    }
}

/// Write wrapper that counts bytes accepted by the wrapped writer.
struct CountingWriter<'a, W> {
    /// Wrapped writer.
    inner: &'a mut W,
    /// Saturating count of bytes written so far.
    written: u64,
}

impl<W: Write> Write for CountingWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let count = self.inner.write(buf)?;
        self.written = self.written.saturating_add(count as u64);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Distinguish a bounded candidate line from an oversized line being skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NinjaStatusLineState {
    /// Retain a bounded candidate that may carry a Ninja status update.
    Buffering,
    /// Discard an oversized line until its terminating newline arrives.
    IgnoringUntilNewline,
}

/// Read wrapper that parses bounded Ninja status lines and reports progress.
///
/// This reader forwards every byte unchanged. It retains at most
/// [`Self::MAX_LINE_BYTES`] for one candidate status line; an oversized line
/// is ignored until its newline, then parsing resumes for the next line. EOF
/// finishes a bounded partial candidate and discards an ignored partial line.
struct NinjaStatusParsingReader<'a, R, F> {
    /// Wrapped reader.
    inner: &'a mut R,
    /// Tracker rejecting regressive or inconsistent status updates.
    tracker: NinjaTaskProgressTracker,
    /// Bytes of the bounded status-line candidate currently being assembled.
    pending_line: Vec<u8>,
    /// Whether the current line remains a bounded parsing candidate.
    line_state: NinjaStatusLineState,
    /// Callback receiving accepted `(current, total, description)` updates.
    observer: &'a mut F,
}

impl<R, F> NinjaStatusParsingReader<'_, R, F> {
    /// Bound one retained candidate to a plausible Ninja status-line length.
    ///
    /// Ninja's default `[current/total] description` format is short, so 512
    /// bytes leaves ample room for its description while preventing child
    /// output without newlines from becoming parent-owned memory.
    const MAX_LINE_BYTES: usize = 512;

    /// Split `bytes` into complete lines, finishing each one when it ends.
    fn consume_bytes(&mut self, bytes: &[u8])
    where
        F: FnMut(u32, u32, &str),
    {
        for byte in bytes {
            if *byte == b'\n' {
                self.finish_line();
            } else if self.line_state == NinjaStatusLineState::Buffering {
                self.retain_candidate_byte(*byte);
            }
        }
    }

    /// Retain `byte` unless doing so would exceed the candidate-line bound.
    fn retain_candidate_byte(&mut self, byte: u8) {
        if self.pending_line.len() < Self::MAX_LINE_BYTES {
            self.pending_line.push(byte);
        } else {
            self.pending_line.clear();
            self.line_state = NinjaStatusLineState::IgnoringUntilNewline;
        }
    }

    /// Report a bounded pending line when it parses and passes the tracker.
    ///
    /// Ignored lines skip UTF-8 decoding entirely, so a long non-status line
    /// cannot allocate a lossy string at newline or EOF.
    fn finish_line(&mut self)
    where
        F: FnMut(u32, u32, &str),
    {
        if self.line_state == NinjaStatusLineState::Buffering
            && let Ok(text) = std::str::from_utf8(&self.pending_line)
            && let Some(progress) = parse_ninja_status_line(text)
            && self.tracker.accept(&progress)
        {
            (self.observer)(progress.current(), progress.total(), progress.description());
        }
        self.pending_line.clear();
        self.line_state = NinjaStatusLineState::Buffering;
    }

    /// Return the number of parser-retained candidate bytes.
    #[cfg(test)]
    #[must_use]
    const fn pending_line_len(&self) -> usize {
        self.pending_line.len()
    }
}

impl<R, F> Read for NinjaStatusParsingReader<'_, R, F>
where
    R: Read,
    F: FnMut(u32, u32, &str),
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buf)?;
        if count == 0 {
            self.finish_line();
            return Ok(0);
        }
        let (slice, _) = buf.split_at(count);
        self.consume_bytes(slice);
        Ok(count)
    }
}

/// Convert a byte count to `usize` with saturation.
fn clamp_u64_to_usize(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

/// Copy `reader` to `writer`, tracking statistics and draining on failure.
fn copy_with_stats<R, W>(reader: &mut R, writer: &mut W, stream_name: &'static str) -> ForwardStats
where
    R: Read,
    W: Write,
{
    let mut stats = ForwardStats::default();
    let mut counting_reader = CountingReader {
        inner: reader,
        read: 0,
    };
    let mut counting_writer = CountingWriter {
        inner: writer,
        written: 0,
    };

    match io::copy(&mut counting_reader, &mut counting_writer) {
        Ok(_) => {
            stats.bytes_read = clamp_u64_to_usize(counting_reader.read);
            stats.bytes_written = clamp_u64_to_usize(counting_writer.written);
        }
        Err(err) => {
            stats.write_failed = true;
            stats.bytes_read = clamp_u64_to_usize(counting_reader.read);
            stats.bytes_written = clamp_u64_to_usize(counting_writer.written);
            tracing::debug!(
                "Failed to write child {stream_name} output to parent: {err}; discarding remaining bytes"
            );
            if let Err(drain_err) = io::copy(&mut counting_reader, &mut io::sink()) {
                tracing::debug!(
                    "Failed to drain child {stream_name} output after writer closed: {drain_err}"
                );
            } else {
                stats.bytes_read = clamp_u64_to_usize(counting_reader.read);
            }
        }
    }
    stats
}

/// Forward child output to a writer while tracking read/write statistics.
pub(super) fn forward_child_output<R, W>(
    mut reader: R,
    mut writer: W,
    stream_name: &'static str,
) -> ForwardStats
where
    R: Read,
    W: Write,
{
    copy_with_stats(&mut reader, &mut writer, stream_name)
}

/// Forward child output and parse Ninja status updates from complete lines.
pub(super) fn forward_child_output_with_ninja_status<R, W, F>(
    mut reader: R,
    mut writer: W,
    mut observer: F,
    stream_name: &'static str,
) -> ForwardStats
where
    R: Read,
    W: Write,
    F: FnMut(u32, u32, &str),
{
    let mut parsing_reader = NinjaStatusParsingReader {
        inner: &mut reader,
        tracker: NinjaTaskProgressTracker::default(),
        pending_line: Vec::with_capacity(NinjaStatusParsingReader::<R, F>::MAX_LINE_BYTES),
        line_state: NinjaStatusLineState::Buffering,
        observer: &mut observer,
    };
    copy_with_stats(&mut parsing_reader, &mut writer, stream_name)
}

#[cfg(test)]
mod tests {
    //! Unit tests for streaming child process output forwarding.
    //!
    //! JSON output and `progress = never` use `forward_child_output` directly,
    //! so these parser checks cover only the progress-enabled stdout path.

    use super::{
        NinjaStatusLineState, NinjaStatusParsingReader, forward_child_output,
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
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            let previous = self.writes.fetch_add(1, Ordering::SeqCst);
            let error_kind = if previous == 0 {
                std::io::ErrorKind::BrokenPipe
            } else {
                std::io::ErrorKind::Other
            };
            Err(std::io::Error::new(error_kind, "sink closed"))
        }

        fn flush(&mut self) -> std::io::Result<()> {
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
        let mut reader = ChunkedReader::new(input.clone(), vec![127; 4096]);
        let mut updates = Vec::new();
        let (forwarded, pending_line_len) = {
            let mut observer = |current, total, description: &str| {
                updates.push((current, total, description.to_owned()));
            };
            let mut parser = NinjaStatusParsingReader {
                inner: &mut reader,
                tracker: Default::default(),
                pending_line: Vec::with_capacity(TEST_MAX_LINE_BYTES),
                line_state: NinjaStatusLineState::Buffering,
                observer: &mut observer,
            };
            let mut forwarded = Vec::new();
            let mut buffer = [0_u8; 1024];

            loop {
                let count = parser
                    .read(&mut buffer)
                    .expect("reader should remain healthy");
                if count == 0 {
                    break;
                }
                let written = buffer
                    .get(..count)
                    .expect("reported read count should fit the supplied buffer");
                forwarded
                    .write_all(written)
                    .expect("vector writer should accept forwarded bytes");
                assert!(
                    parser.pending_line_len() <= TEST_MAX_LINE_BYTES,
                    "status parsing must not retain an unbounded partial line"
                );
            }

            (forwarded, parser.pending_line_len())
        };

        assert_eq!(forwarded, input);
        assert!(updates.is_empty());
        assert_eq!(pending_line_len, 0);
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
                tracker: Default::default(),
                pending_line: Vec::with_capacity(TEST_MAX_LINE_BYTES),
                line_state: NinjaStatusLineState::Buffering,
                observer: &mut observer,
            };
            let mut forwarded = Vec::new();
            let mut buffer = [0_u8; 128];

            loop {
                let count = parser.read(&mut buffer).expect("generated reader should remain healthy");
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
        let status_before =
            peak_rss_bytes().expect("getrusage should report status forwarding RSS");
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
}
