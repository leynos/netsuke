//! Streaming helpers for subprocess output forwarding.

use super::ninja_status::{NinjaTaskProgressTracker, parse_ninja_status_line};
use std::io::{self, Read, Write};

/// Forwarding statistics for a child output stream.
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub(super) struct ForwardStats {
    pub(super) bytes_read: usize,
    pub(super) bytes_written: usize,
    pub(super) write_failed: bool,
}

struct CountingReader<'a, R> {
    inner: &'a mut R,
    read: u64,
}

impl<R: Read> Read for CountingReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buf)?;
        self.read = self.read.saturating_add(count as u64);
        Ok(count)
    }
}

struct CountingWriter<'a, W> {
    inner: &'a mut W,
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

struct NinjaStatusParsingReader<'a, R, F> {
    inner: &'a mut R,
    tracker: NinjaTaskProgressTracker,
    pending_line: Vec<u8>,
    observer: &'a mut F,
}

impl<R, F> NinjaStatusParsingReader<'_, R, F> {
    fn consume_bytes(&mut self, bytes: &[u8])
    where
        F: FnMut(u32, u32, &str),
    {
        for byte in bytes {
            if *byte == b'\n' {
                self.finish_line();
            } else {
                self.pending_line.push(*byte);
            }
        }
    }

    fn finish_line(&mut self)
    where
        F: FnMut(u32, u32, &str),
    {
        if self.pending_line.is_empty() {
            return;
        }
        let text = String::from_utf8_lossy(&self.pending_line);
        if let Some(progress) = parse_ninja_status_line(text.as_ref())
            && self.tracker.accept(&progress)
        {
            (self.observer)(progress.current(), progress.total(), progress.description());
        }
        self.pending_line.clear();
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
        let slice = buf
            .get(..count)
            .ok_or_else(|| io::Error::other("reader returned out-of-range byte count"))?;
        self.consume_bytes(slice);
        Ok(count)
    }
}

fn clamp_u64_to_usize(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

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
        pending_line: Vec::new(),
        observer: &mut observer,
    };
    copy_with_stats(&mut parsing_reader, &mut writer, stream_name)
}

#[cfg(test)]
mod tests {
    use super::{forward_child_output, forward_child_output_with_ninja_status};
    use std::{
        io::{BufReader, Cursor, Write},
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
}
