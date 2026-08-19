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

/// The most startup diagnostics that will be held before the effective mode is
/// known.
///
/// The window is short — locale resolution, then command-line parsing — and
/// what it carries is a handful of one-line warnings. The bound exists so that
/// the size of what is buffered never depends on how much a run happens to
/// emit: without it, a pathological run could hold arbitrary bytes in memory
/// before anything decided where they belong.
///
/// Overflow keeps the first bytes rather than the last: the earliest
/// diagnostics describe how the run was configured, which is what a reader
/// needs, and later ones are progressively less informative about the startup
/// decision. Once full, [`TRUNCATION_MARKER`] is appended once, if it fits, and
/// everything after is dropped.
const MAX_BUFFERED_BYTES: usize = 64 * 1024;

/// Appended once when [`MAX_BUFFERED_BYTES`] is reached, so a truncated buffer
/// says so rather than appearing to be the whole of it.
const TRUNCATION_MARKER: &[u8] = b"\n[startup diagnostics truncated]\n";

/// Where startup diagnostics are going right now.
enum Sink {
    /// Held until the effective mode is known.
    Buffered(BoundedBuffer),
    /// Human mode: written through to stderr.
    Stderr,
    /// JSON mode: discarded, so stderr carries only the diagnostic document.
    Discard,
}

/// A byte buffer that stops growing at [`MAX_BUFFERED_BYTES`].
///
/// Once full it records that it truncated, so the marker is appended exactly
/// once however many writes follow.
#[derive(Default)]
struct BoundedBuffer {
    /// Bytes held so far, bounded by `MAX_BUFFERED_BYTES`.
    bytes: Vec<u8>,
    /// Whether the bound was reached and the truncation marker appended.
    truncated: bool,
}

impl BoundedBuffer {
    /// Append as much of `buf` as the bound allows.
    ///
    /// The first write to overflow keeps the bytes that fit, then appends the
    /// truncation marker if there is room for it. Later writes are dropped.
    fn append(&mut self, buf: &[u8]) {
        if self.truncated {
            return;
        }
        let remaining = MAX_BUFFERED_BYTES.saturating_sub(self.bytes.len());
        if buf.len() <= remaining {
            self.bytes.extend_from_slice(buf);
            return;
        }
        // Room for the marker is reserved rather than claimed afterwards. A
        // buffer filled to exactly the bound would leave none, and the marker
        // would be dropped in precisely the case it is needed.
        let content_limit = MAX_BUFFERED_BYTES.saturating_sub(TRUNCATION_MARKER.len());
        if self.bytes.len() < content_limit {
            let keep = content_limit.saturating_sub(self.bytes.len());
            // `get` rather than a slice index: the lint forbids slicing that
            // could panic, and a short `buf` is a legitimate input here.
            if let Some(head) = buf.get(..keep.min(buf.len())) {
                self.bytes.extend_from_slice(head);
            }
        }
        self.bytes.truncate(content_limit);
        self.bytes.extend_from_slice(TRUNCATION_MARKER);
        self.truncated = true;
    }

    /// Drain the held bytes, resetting the truncation state.
    fn take(&mut self) -> Vec<u8> {
        self.truncated = false;
        std::mem::take(&mut self.bytes)
    }

    #[cfg(test)]
    fn as_slice(&self) -> &[u8] {
        &self.bytes
    }
}

/// A writer that buffers until told where the output belongs.
///
/// Cloned by the `fmt` layer for each event, so the sink is shared behind an
/// `Arc`; every clone observes the same state and the same buffer.
#[derive(Clone)]
pub struct StartupWriter {
    /// Shared sink state visible to every clone of the writer.
    sink: Arc<Mutex<Sink>>,
}

impl StartupWriter {
    /// A writer that holds everything written to it.
    ///
    /// # Examples
    ///
    /// ```text
    /// let w = StartupWriter::buffering();
    /// warn!("locale fell back");   ->  held; nothing reaches stderr
    /// ```
    ///
    /// Examples are shown rather than run: this module is compiled into the
    /// binary, and Cargo does not run doctests for a binary target, so a
    /// `rust` block would never be checked and would rot unnoticed.
    #[must_use]
    pub fn buffering() -> Self {
        Self {
            sink: Arc::new(Mutex::new(Sink::Buffered(BoundedBuffer::default()))),
        }
    }

    /// Lock the shared sink, recovering from a poisoned mutex.
    fn lock(&self) -> MutexGuard<'_, Sink> {
        // A panic while formatting an event must not cascade into losing the
        // rest of the diagnostics.
        self.sink.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Write everything buffered to stderr, and write through from now on.
    ///
    /// Called when the effective mode turns out to be human.
    ///
    /// # Examples
    ///
    /// ```text
    /// warn!("locale fell back");   ->  held
    /// w.release_to_stderr()?;      ->  the held bytes reach stderr, buffer emptied
    /// warn!("something later");    ->  written straight to stderr
    /// ```
    ///
    /// # Errors
    ///
    /// Returns the error from writing the buffered bytes to stderr.
    pub fn release_to_stderr(&self) -> io::Result<()> {
        let mut sink = self.lock();
        let buffered = match &mut *sink {
            Sink::Buffered(buffer) => buffer.take(),
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
    ///
    /// Called when the effective mode turns out to be JSON, so that stderr
    /// carries only the diagnostic document.
    ///
    /// # Examples
    ///
    /// ```text
    /// warn!("locale fell back");   ->  held
    /// w.discard();                 ->  the held bytes are dropped
    /// warn!("something later");    ->  dropped too, not re-buffered
    /// ```
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
            Sink::Buffered(buffer) => buffer.as_slice().to_vec(),
            Sink::Stderr | Sink::Discard => Vec::new(),
        }
    }
}

/// The per-event handle the `fmt` layer writes through.
pub struct StartupWriterHandle {
    /// Shared sink state this handle forwards events to.
    sink: Arc<Mutex<Sink>>,
}

impl Write for StartupWriterHandle {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut sink = self.sink.lock().unwrap_or_else(PoisonError::into_inner);
        match &mut *sink {
            Sink::Buffered(buffer) => {
                buffer.append(buf);
                // The whole slice is reported as written even when the bound
                // dropped some of it: the formatter has no recourse, and a
                // short write would be reported through the channel being
                // truncated.
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
