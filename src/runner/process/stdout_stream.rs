//! Streaming writer for text that a command emits to standard output.
//!
//! `netsuke help targets` can emit far more bytes than its manifest occupies
//! when one description is shared by many target names. Assembling that
//! catalogue in memory would multiply peak allocation by the number of names,
//! so the renderer streams rows instead. This module owns the stdout seam: a
//! buffered writer that tolerates a closed pipe and reports every other write
//! failure to the caller.

use anyhow::{Context, Result};
use std::io::{self, BufWriter, Write};

use crate::localization::{self, keys};

/// Stream text to stdout through a buffered, closed-pipe-tolerant writer.
///
/// The closure receives the writer and may emit any number of chunks. Each
/// chunk reaches stdout as the renderer produces it, so peak memory follows the
/// largest chunk rather than the accumulated output.
///
/// # Errors
///
/// Returns an error when the renderer fails or a stdout write fails for a
/// reason other than a broken pipe.
pub fn stream_text_stdout(render: impl FnOnce(&mut dyn Write) -> Result<()>) -> Result<()> {
    let stdout = io::stdout();
    let mut buffered = BufWriter::new(stdout.lock());
    let mut sink = ClosedPipeWriter::new(&mut buffered);
    render(&mut sink)?;
    sink.flush()
        .context(localization::message(keys::RUNNER_IO_FLUSH_STDOUT))?;
    Ok(())
}

/// Write through to the inner writer, treating a closed pipe as a no-op.
///
/// Once the reader closes the pipe, later chunks are accepted without an
/// attempt to write them, so a consumer such as `head` ends the stream without
/// surfacing an error or leaving an oversized chunk to allocate.
struct ClosedPipeWriter<W> {
    /// Destination that receives each chunk.
    inner: W,
    /// Whether the reader has closed the pipe.
    closed: bool,
}

impl<W> ClosedPipeWriter<W> {
    /// Wrap `inner`, starting in the open state.
    const fn new(inner: W) -> Self {
        Self {
            inner,
            closed: false,
        }
    }
}

impl<W: Write> Write for ClosedPipeWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.closed {
            return Ok(buf.len());
        }
        match self.inner.write(buf) {
            Ok(written) => Ok(written),
            Err(error) if super::file_io::is_broken_pipe(&error) => {
                self.closed = true;
                Ok(buf.len())
            }
            Err(error) => Err(error),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        match self.inner.flush() {
            Ok(()) => Ok(()),
            Err(error) if super::file_io::is_broken_pipe(&error) => {
                self.closed = true;
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Pin closed-pipe tolerance and pass-through of other failures.

    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Return a writer that always reports a broken pipe and counts attempts.
    ///
    /// Counting is what distinguishes accepting the rest of the stream after
    /// the first failure from writing to a closed pipe on every chunk.
    struct ClosedPipe {
        /// Number of write attempts the sink has made.
        writes: Arc<AtomicUsize>,
    }

    impl ClosedPipe {
        /// Build the writer with a fresh attempt counter.
        fn new() -> Self {
            Self {
                writes: Arc::new(AtomicUsize::new(0)),
            }
        }

        /// Share the counter so a test can read it after the sink takes the
        /// writer.
        fn counter(&self) -> Arc<AtomicUsize> {
            Arc::clone(&self.writes)
        }
    }

    impl Write for ClosedPipe {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            self.writes.fetch_add(1, Ordering::SeqCst);
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }
    }

    /// Return a writer that fails with a non-pipe error.
    struct BrokenDisk;

    impl Write for BrokenDisk {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::StorageFull))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// Accept every chunk after the reader closes the pipe without writing it.
    #[test]
    fn accepts_chunks_after_a_broken_pipe() -> Result<()> {
        let pipe = ClosedPipe::new();
        let attempts = pipe.counter();
        let mut sink = ClosedPipeWriter::new(pipe);

        sink.write_all(b"first row")?;
        sink.write_all(b"second row")?;
        sink.flush()?;

        anyhow::ensure!(
            attempts.load(Ordering::SeqCst) == 1,
            "only the first chunk should reach the closed writer, saw {} attempts",
            attempts.load(Ordering::SeqCst)
        );
        Ok(())
    }

    /// Propagate a failure that is not a closed pipe.
    #[test]
    fn reports_other_write_failures() {
        let mut sink = ClosedPipeWriter::new(BrokenDisk);
        let error = sink
            .write_all(b"first row")
            .expect_err("storage failures must reach the caller");
        assert_eq!(error.kind(), io::ErrorKind::StorageFull);
    }
}
