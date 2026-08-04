//! Startup diagnostics held until the effective output mode is known.
//!
//! Netsuke resolves its locale before it parses the command line, because
//! usage errors have to be rendered in the user's language. That ordering
//! creates a window: a locale that falls back to English is worth reporting,
//! but the JSON diagnostic document is written to stderr, and configuration can
//! still turn JSON on after the fallback has happened. Emitting immediately
//! risks corrupting that document; emitting at `OFF` loses the report.
//!
//! So startup events are written to a buffer instead of a stream. Once the
//! effective mode is settled, the buffer is either released to stderr (human
//! mode) or dropped (JSON mode), and everything after that is written straight
//! through.

use std::io::{self, Write};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tracing_subscriber::fmt::MakeWriter;

/// Where startup diagnostics are going right now.
enum Sink {
    /// Held until the effective mode is known.
    Buffered(Vec<u8>),
    /// Human mode: written through to stderr.
    Stderr,
    /// JSON mode: discarded, so stderr carries only the diagnostic document.
    Discard,
}

/// A writer that buffers until told where the output belongs.
///
/// Cloned by the `fmt` layer for each event, so the sink is shared behind an
/// `Arc`; every clone observes the same state and the same buffer.
#[derive(Clone)]
pub struct StartupWriter {
    sink: Arc<Mutex<Sink>>,
}

impl StartupWriter {
    /// A writer that holds everything written to it.
    #[must_use]
    pub fn buffering() -> Self {
        Self {
            sink: Arc::new(Mutex::new(Sink::Buffered(Vec::new()))),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Sink> {
        // A panic while formatting an event must not cascade into losing the
        // rest of the diagnostics.
        self.sink.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Write everything buffered to stderr, and write through from now on.
    ///
    /// # Errors
    ///
    /// Returns the error from writing the buffered bytes to stderr.
    pub fn release_to_stderr(&self) -> io::Result<()> {
        let mut sink = self.lock();
        let buffered = match &mut *sink {
            Sink::Buffered(bytes) => std::mem::take(bytes),
            Sink::Stderr | Sink::Discard => Vec::new(),
        };
        *sink = Sink::Stderr;
        drop(sink);
        if buffered.is_empty() {
            return Ok(());
        }
        io::stderr().write_all(&buffered)
    }

    /// Drop everything buffered, and discard whatever follows.
    pub fn discard(&self) {
        let mut sink = self.lock();
        *sink = Sink::Discard;
    }

    /// The bytes currently held, for tests that assert what was recorded
    /// before the mode was known.
    ///
    /// Test-only: production code never inspects the buffer, it only decides
    /// where the buffer goes.
    #[cfg(test)]
    #[must_use]
    pub fn buffered(&self) -> Vec<u8> {
        match &*self.lock() {
            Sink::Buffered(bytes) => bytes.clone(),
            Sink::Stderr | Sink::Discard => Vec::new(),
        }
    }
}

/// The per-event handle the `fmt` layer writes through.
pub struct StartupWriterHandle {
    sink: Arc<Mutex<Sink>>,
}

impl Write for StartupWriterHandle {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut sink = self.sink.lock().unwrap_or_else(PoisonError::into_inner);
        match &mut *sink {
            Sink::Buffered(bytes) => {
                bytes.extend_from_slice(buf);
                Ok(buf.len())
            }
            Sink::Stderr => {
                drop(sink);
                io::stderr().write(buf)
            }
            // Report the bytes as written: the caller has no recourse, and a
            // short-write error would be reported through the very channel
            // being discarded.
            Sink::Discard => Ok(buf.len()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        let sink = self.sink.lock().unwrap_or_else(PoisonError::into_inner);
        if matches!(&*sink, Sink::Stderr) {
            drop(sink);
            return io::stderr().flush();
        }
        Ok(())
    }
}

impl<'writer> MakeWriter<'writer> for StartupWriter {
    type Writer = StartupWriterHandle;

    fn make_writer(&'writer self) -> Self::Writer {
        StartupWriterHandle {
            sink: Arc::clone(&self.sink),
        }
    }
}

#[cfg(test)]
#[path = "startup_tracing_tests.rs"]
mod tests;
