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
