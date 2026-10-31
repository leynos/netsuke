//! Streaming helpers for subprocess output forwarding.
//!
//! The module owns the only byte-for-byte copy loop between a child's standard
//! streams and a forwarding writer. See `forward_failure` for the closed
//! failure vocabulary that keeps forwarding telemetry bounded.

use super::ninja::{NinjaTaskProgressTracker, parse_ninja_status_line};
use metrics::{counter, describe_counter};
use std::{
    io::{self, Read, Write},
    sync::Once,
};

mod forward_failure;
pub use forward_failure::{FORWARD_FAILURE_LABELS, FORWARD_FAILURES_TOTAL};
use forward_failure::{ForwardFailure, record_failure};
pub(super) use forward_failure::{ForwardFailureKind, ForwardStream};

/// Forwarding statistics for a child output stream.
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub(super) struct ForwardStats {
    /// Bytes consumed from the child stream.
    pub(super) bytes_read: usize,
    /// Bytes accepted by the forwarding writer.
    pub(super) bytes_written: usize,
    /// Classification of the first forwarding failure, if forwarding failed.
    pub(super) failure: Option<ForwardFailureKind>,
    /// Whether draining the remaining child output failed after that failure.
    pub(super) drain_failed: bool,
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

/// Count status lines skipped because they exceed the retained candidate bound.
pub const NINJA_STATUS_OVERSIZED_LINES_TOTAL: &str = "netsuke_ninja_status_oversized_lines_total";

/// Describe the oversized-status-line counter once per process.
fn describe_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            NINJA_STATUS_OVERSIZED_LINES_TOTAL,
            "Counts Ninja status lines skipped after exceeding the retained candidate bound."
        );
    });
}

/// Record one oversized status line without retaining its output.
fn record_oversized_status_line() {
    describe_metrics();
    counter!(NINJA_STATUS_OVERSIZED_LINES_TOTAL).increment(1);
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
            record_oversized_status_line();
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
        if self.line_state == NinjaStatusLineState::Buffering {
            self.report_pending_line();
        }
        self.pending_line.clear();
        self.line_state = NinjaStatusLineState::Buffering;
    }

    /// Notify the observer when the bounded line carries accepted progress.
    fn report_pending_line(&mut self)
    where
        F: FnMut(u32, u32, &str),
    {
        let Ok(text) = std::str::from_utf8(&self.pending_line) else {
            return;
        };
        let Some(progress) = parse_ninja_status_line(text) else {
            return;
        };
        if self.tracker.accept(&progress) {
            (self.observer)(progress.current(), progress.total(), progress.description());
        }
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

/// Read one child-output chunk, retrying interrupted reads.
///
/// # Errors
///
/// Returns an error when the reader fails.
fn read_next_chunk<R>(reader: &mut R, buffer: &mut [u8]) -> io::Result<usize>
where
    R: Read,
{
    loop {
        match reader.read(buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            result => break result,
        }
    }
}

/// Write a complete child-output chunk and flush it to the destination.
///
/// # Errors
///
/// Returns a classified failure when the writer rejects the chunk or its
/// flush.
fn write_and_flush_chunk<W>(writer: &mut W, chunk: &[u8]) -> Result<(), ForwardFailure>
where
    W: Write,
{
    writer.write_all(chunk).map_err(ForwardFailure::write)?;
    writer.flush().map_err(ForwardFailure::flush)
}

/// Copy and flush each child-read chunk before reading the next one.
///
/// Keeping the flush separate from `Write::write` ensures a flush failure
/// cannot cause accepted bytes to be retried as another write.
///
/// # Errors
///
/// Returns the first failure encountered while reading, writing, or flushing,
/// classified by stage. A reader that reports more bytes than the buffer holds
/// is a read failure: the count is only meaningful as a read result.
fn copy_and_flush<R, W>(reader: &mut R, writer: &mut W) -> Result<(), ForwardFailure>
where
    R: Read,
    W: Write,
{
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let count = read_next_chunk(reader, &mut buffer).map_err(ForwardFailure::read)?;
        if count == 0 {
            return Ok(());
        }
        let chunk = buffer.get(..count).ok_or_else(|| {
            ForwardFailure::read(io::Error::other("reader reported an invalid byte count"))
        })?;
        write_and_flush_chunk(writer, chunk)?;
    }
}

/// Copy `reader` to `writer`, tracking statistics and draining on failure.
///
/// Byte counts are recorded on every path, including a failed drain, so they
/// always describe bytes the forwarder actually consumed and accepted.
fn copy_with_stats<R, W>(reader: &mut R, writer: &mut W, stream: ForwardStream) -> ForwardStats
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

    if let Err(failure) = copy_and_flush(&mut counting_reader, &mut counting_writer) {
        stats.failure = Some(failure.kind);
        record_failure(stream, failure.kind);
        tracing::warn!(
            stream = stream.label(),
            failure_kind = failure.kind.label(),
            error = %failure.source,
            "failed to forward child output; discarding the remaining bytes"
        );
        stats.drain_failed = drain_remaining(&mut counting_reader, stream);
    }

    stats.bytes_read = clamp_u64_to_usize(counting_reader.read);
    stats.bytes_written = clamp_u64_to_usize(counting_writer.written);
    stats
}

/// Drain the remaining child output after a forwarding failure.
///
/// Draining keeps the child from blocking on a full pipe once the parent has
/// stopped consuming its output. A drain failure is recorded under its own
/// classification so it never replaces the failure that stopped forwarding.
fn drain_remaining<R>(reader: &mut CountingReader<'_, R>, stream: ForwardStream) -> bool
where
    R: Read,
{
    match io::copy(reader, &mut io::sink()) {
        Ok(_) => false,
        Err(error) => {
            record_failure(stream, ForwardFailureKind::Drain);
            tracing::warn!(
                stream = stream.label(),
                failure_kind = ForwardFailureKind::Drain.label(),
                error = %error,
                "failed to drain child output after a forwarding failure"
            );
            true
        }
    }
}

/// Forward child output to a writer while tracking read/write statistics.
pub(super) fn forward_child_output<R, W>(
    mut reader: R,
    mut writer: W,
    stream: ForwardStream,
) -> ForwardStats
where
    R: Read,
    W: Write,
{
    copy_with_stats(&mut reader, &mut writer, stream)
}

/// Forward child output and parse Ninja status updates from complete lines.
pub(super) fn forward_child_output_with_ninja_status<R, W, F>(
    mut reader: R,
    mut writer: W,
    mut observer: F,
    stream: ForwardStream,
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
    copy_with_stats(&mut parsing_reader, &mut writer, stream)
}

#[cfg(test)]
mod telemetry_tests;
#[cfg(test)]
mod tests;
