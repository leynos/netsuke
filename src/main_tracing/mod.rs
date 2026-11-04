//! Initialize and adjust the process-wide tracing subscriber for the binary.
//!
//! The binary buffers early events until configuration resolves the effective
//! diagnostic mode, then changes the subscriber's filter without installing a
//! second global subscriber.

use std::io::{self, IsTerminal};
use std::sync::OnceLock;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{Registry, fmt, reload};

use crate::startup_tracing::StartupWriter;

/// Name the handle used to adjust the subscriber's level after startup.
type TracingFilterHandle = reload::Handle<LevelFilter, Registry>;

/// Store the handle for adjusting the installed subscriber's level.
static TRACING_FILTER: OnceLock<TracingFilterHandle> = OnceLock::new();

/// Install the process-wide subscriber with a reloadable filter set to `initial`.
///
/// Only the first call installs; later calls are ignored so exactly one global
/// subscriber exists. Events go to `writer`, which buffers until the effective
/// mode is known and then releases them to stderr or discards them, never
/// stdout, so a JSON document is not interleaved.
///
/// # Examples
///
/// ```text
/// init_tracing(LevelFilter::WARN, StartupWriter::buffering());
/// ```
pub(super) fn init_tracing(initial: LevelFilter, writer: StartupWriter) {
    let (filter, handle) = reload::Layer::new(initial);
    if Registry::default()
        .with(filter)
        .with(
            fmt::layer()
                .with_writer(writer)
                // Colour only a terminal; piped or redirected logs stay plain
                // so they remain greppable and free of escape sequences.
                .with_ansi(io::stderr().is_terminal()),
        )
        .try_init()
        .is_ok()
    {
        TRACING_FILTER.set(handle).ok();
    }
}

/// Adjust the installed subscriber's level when one was installed.
///
/// # Examples
///
/// ```text
/// set_tracing_filter(LevelFilter::TRACE);
/// ```
pub(super) fn set_tracing_filter(level: LevelFilter) {
    if let Some(handle) = TRACING_FILTER.get() {
        handle.modify(|filter| *filter = level).ok();
    }
}
