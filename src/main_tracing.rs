//! Initialize and adjust the process-wide tracing subscriber for the binary.
//!
//! The binary buffers early events until configuration resolves the effective
//! diagnostic mode, then changes the subscriber's filter without installing a
//! second global subscriber.
//!
//! The two halves of that handshake live either side of this module. The writer
//! is a [`StartupWriter`], which is where the buffered events actually sit:
//! [`init_tracing`] is handed one by [`crate::main`] and treats it as opaque,
//! so this module decides where events go while `startup_tracing` decides how
//! long they are held and where they are released.
//!
//! Settling is not this module's decision and it is not a single moment.
//! [`crate::config_load`] settles the buffer after diagnostic-mode resolution
//! succeeds and *before* the merge, so the startup warning reaches the user
//! ahead of any configuration processing, and again when that resolution fails.
//! [`crate::main_startup::parse_cli_or_exit`] settles it on every parse exit,
//! because help, version and usage errors all terminate there. [`crate::main`]
//! settles the informational-help path, which never reaches either. Only the
//! ANSI reload is unconditional and ordered: it happens once the plan is
//! resolved and before anything is styled with it.

use std::sync::OnceLock;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{Registry, fmt, reload};

use crate::startup_tracing::StartupWriter;

/// Name the handle used to adjust the subscriber's level after startup.
type TracingFilterHandle = reload::Handle<LevelFilter, Registry>;

/// Set the ANSI mode on the installed formatting layer.
type TracingAnsiSetter = Box<dyn Fn(bool) + Send + Sync>;

/// Store the handle for adjusting the installed subscriber's level.
static TRACING_FILTER: OnceLock<TracingFilterHandle> = OnceLock::new();

/// Store the setter for adjusting the installed formatting layer's ANSI mode.
static TRACING_ANSI: OnceLock<TracingAnsiSetter> = OnceLock::new();

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
    let (formatting, ansi_handle) = reload::Layer::new(
        fmt::layer()
            .with_writer(writer)
            // Startup starts plain until the display policy is resolved.
            .with_ansi(false),
    );
    if Registry::default()
        .with(filter)
        .with(formatting)
        .try_init()
        .is_ok()
    {
        TRACING_FILTER.set(handle).ok();
        TRACING_ANSI
            .set(Box::new(move |ansi| {
                ansi_handle.modify(|layer| layer.set_ansi(ansi)).ok();
            }))
            .ok();
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

/// Adjust the installed formatting layer when one was installed.
///
/// # Examples
///
/// ```text
/// set_tracing_ansi(true);
/// ```
pub(super) fn set_tracing_ansi(ansi: bool) {
    if let Some(setter) = TRACING_ANSI.get() {
        setter(ansi);
    }
}
