//! Tests for the buffered startup writer.
//!
//! These drive a real `fmt` layer through a scoped subscriber, so what is
//! asserted is what the layer actually writes, not a stand-in for it.

use super::*;
use anyhow::{Result, ensure};
use tracing_subscriber::{filter::LevelFilter, fmt, prelude::*, registry::Registry};

/// Emit `event` through a subscriber writing to `writer`.
///
/// Scoped with `with_default` rather than installed globally, so each test gets
/// its own subscriber and they do not contend for the process-wide one.
fn emit_warning(writer: &StartupWriter, message: &'static str) {
    let subscriber = Registry::default()
        .with(LevelFilter::WARN)
        .with(fmt::layer().with_writer(writer.clone()).with_ansi(false));
    tracing::subscriber::with_default(subscriber, || {
        tracing::warn!(target: "startup", "{message}");
    });
}

/// A warning emitted before the mode is known must be held, not written.
#[test]
fn a_startup_warning_is_buffered_rather_than_written() -> Result<()> {
    let writer = StartupWriter::buffering();
    emit_warning(&writer, "locale fell back");

    let buffered = String::from_utf8(writer.buffered())?;
    ensure!(
        buffered.contains("locale fell back"),
        "the event must be recorded while buffering, got {buffered:?}"
    );
    Ok(())
}

/// Releasing empties the buffer, so nothing is written twice when later events
/// pass straight through.
#[test]
fn releasing_empties_the_buffer() -> Result<()> {
    let writer = StartupWriter::buffering();
    emit_warning(&writer, "locale fell back");
    ensure!(!writer.buffered().is_empty(), "expected a buffered event");

    writer.release_to_stderr()?;

    ensure!(
        writer.buffered().is_empty(),
        "the buffer must be emptied once released"
    );
    Ok(())
}

/// JSON mode drops what was buffered, so stderr carries only the diagnostic
/// document.
#[test]
fn discarding_drops_the_buffer() -> Result<()> {
    let writer = StartupWriter::buffering();
    emit_warning(&writer, "locale fell back");
    ensure!(!writer.buffered().is_empty(), "expected a buffered event");

    writer.discard();

    ensure!(
        writer.buffered().is_empty(),
        "discarding must drop what was buffered"
    );
    Ok(())
}

/// After discarding, later events are dropped too — not buffered up again.
///
/// Without this, a JSON run would accumulate every subsequent event in memory
/// and the mode decision would apply only to the startup window.
#[test]
fn events_after_discarding_are_not_buffered() -> Result<()> {
    let writer = StartupWriter::buffering();
    writer.discard();

    emit_warning(&writer, "after the decision");

    ensure!(
        writer.buffered().is_empty(),
        "an event after discarding must not be retained"
    );
    Ok(())
}

/// Releasing an empty buffer is not an error, which is the common case: most
/// runs resolve their locale without falling back.
#[test]
fn releasing_an_empty_buffer_succeeds() -> Result<()> {
    let writer = StartupWriter::buffering();
    writer.release_to_stderr()?;
    ensure!(
        writer.buffered().is_empty(),
        "an empty buffer stays empty once released"
    );
    Ok(())
}

/// Write `bytes` straight through the writer, bypassing the tracing layer.
///
/// The bound is a property of the writer, not of any event, so these drive it
/// directly rather than trying to provoke a large event.
fn write_raw(writer: &StartupWriter, bytes: &[u8]) -> Result<usize> {
    use std::io::Write as _;
    let mut handle = writer.make_writer();
    Ok(handle.write(bytes)?)
}

/// Below the bound, everything is kept and nothing is marked.
#[test]
fn a_write_below_the_limit_is_kept_whole() -> Result<()> {
    let writer = StartupWriter::buffering();
    let payload = vec![b'a'; MAX_BUFFERED_BYTES - 1];

    let written = write_raw(&writer, &payload)?;

    ensure!(written == payload.len(), "the whole slice must be reported");
    let buffered = writer.buffered();
    ensure!(buffered == payload, "the bytes must be kept unchanged");
    ensure!(
        !buffered.ends_with(TRUNCATION_MARKER),
        "nothing was dropped, so nothing should be marked"
    );
    Ok(())
}

/// Exactly at the bound is not an overflow.
#[test]
fn a_write_at_the_limit_is_kept_whole() -> Result<()> {
    let writer = StartupWriter::buffering();
    let payload = vec![b'a'; MAX_BUFFERED_BYTES];

    write_raw(&writer, &payload)?;

    let buffered = writer.buffered();
    ensure!(
        buffered.len() == MAX_BUFFERED_BYTES,
        "expected exactly the bound, got {}",
        buffered.len()
    );
    ensure!(
        !buffered.ends_with(TRUNCATION_MARKER),
        "a write that fits exactly must not be marked as truncated"
    );
    Ok(())
}

/// Overflow keeps the first bytes, marks the truncation, and never exceeds the
/// bound.
#[test]
fn an_overflowing_write_keeps_the_first_bytes_and_marks_it() -> Result<()> {
    let writer = StartupWriter::buffering();
    let payload = vec![b'a'; MAX_BUFFERED_BYTES + 4096];

    let written = write_raw(&writer, &payload)?;

    ensure!(
        written == payload.len(),
        "dropped overflow must still report the whole slice as written"
    );
    let buffered = writer.buffered();
    ensure!(
        buffered.len() <= MAX_BUFFERED_BYTES,
        "the buffer must not exceed its bound, got {}",
        buffered.len()
    );
    ensure!(
        buffered.starts_with(b"aaaa"),
        "the earliest bytes are the ones kept"
    );
    ensure!(
        buffered.ends_with(TRUNCATION_MARKER),
        "an overflow must be marked"
    );
    Ok(())
}

/// Repeated overflow adds nothing further, and marks once only.
#[test]
fn repeated_overflow_marks_once_and_grows_no_further() -> Result<()> {
    let writer = StartupWriter::buffering();
    write_raw(&writer, &vec![b'a'; MAX_BUFFERED_BYTES + 1])?;
    let after_first = writer.buffered();

    for _ in 0..3 {
        write_raw(&writer, b"more diagnostics that must be dropped")?;
    }
    let after_more = writer.buffered();

    ensure!(
        after_more == after_first,
        "writes after truncation must change nothing"
    );
    let marker = String::from_utf8_lossy(TRUNCATION_MARKER).into_owned();
    let rendered = String::from_utf8_lossy(&after_more).into_owned();
    ensure!(
        rendered.matches(&marker).count() == 1,
        "the marker must appear exactly once, found {}",
        rendered.matches(&marker).count()
    );
    Ok(())
}

/// Releasing after truncation empties the buffer and resumes write-through, so
/// a truncated startup does not stay truncated for the rest of the run.
#[test]
fn releasing_after_truncation_empties_the_buffer() -> Result<()> {
    let writer = StartupWriter::buffering();
    write_raw(&writer, &vec![b'a'; MAX_BUFFERED_BYTES + 1])?;

    writer.release_to_stderr()?;

    ensure!(
        writer.buffered().is_empty(),
        "the buffer must be emptied once released"
    );
    Ok(())
}

/// Discarding after truncation drops everything, as it does untruncated.
#[test]
fn discarding_after_truncation_drops_the_buffer() -> Result<()> {
    let writer = StartupWriter::buffering();
    write_raw(&writer, &vec![b'a'; MAX_BUFFERED_BYTES + 1])?;

    writer.discard();

    ensure!(
        writer.buffered().is_empty(),
        "discarding must drop a truncated buffer too"
    );
    Ok(())
}
