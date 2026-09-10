//! Wall-clock seam for the stdlib `now()` helper.
//!
//! The `now()` helper reports the current instant. Reading it from the host
//! clock directly makes every render that calls `now()` unrepeatable, so the
//! read is instead supplied by the caller as a [`ClockProvider`]. A caller
//! that supplies nothing keeps the ambient behaviour: [`WallClock::default`]
//! installs [`system_clock`], the production adapter.

use std::{fmt, sync::Arc};

use time::{OffsetDateTime, UtcOffset};

/// Re-exported so an external caller can name a provider's return type
/// without adding its own `time` dependency.
///
/// # Examples
///
/// ```
/// use netsuke::stdlib::ClockInstant;
/// use time::{UtcOffset, macros::datetime};
///
/// let instant: ClockInstant = datetime!(2026-06-08 12:00:00 UTC);
/// assert_eq!(instant.offset(), UtcOffset::UTC);
/// ```
pub use time::OffsetDateTime as ClockInstant;

/// Thread-safe wall-clock source supplied to the `now()` Jinja helper.
///
/// The provider is an `Arc` rather than a `Box` for two reasons, both binding:
/// `minijinja` requires registered functions to be `Send + Sync`, and
/// `StdlibConfig` derives `Clone`, which `Box<dyn Fn>` cannot satisfy.
/// ADR-008 records the same shape for the manifest environment reader.
///
/// # Examples
///
/// A caller may hand-write a provider rather than use one of the supplied
/// adapters:
///
/// ```rust
/// use netsuke::stdlib::{ClockInstant, ClockProvider};
/// use std::sync::Arc;
/// use time::macros::datetime;
///
/// let instant = datetime!(2026-06-08 12:00:00 UTC);
/// let clock: ClockProvider = Arc::new(move || instant);
/// let read: ClockInstant = clock();
/// assert_eq!(read, instant);
/// ```
pub type ClockProvider = Arc<dyn Fn() -> OffsetDateTime + Send + Sync>;

/// Construct the host-backed clock provider used by production renders.
///
/// # Examples
///
/// ```rust
/// use netsuke::stdlib::{ClockProvider, system_clock};
/// use time::UtcOffset;
///
/// let clock: ClockProvider = system_clock();
/// assert_eq!(clock().offset(), UtcOffset::UTC);
/// ```
#[must_use]
pub fn system_clock() -> ClockProvider {
    Arc::new(OffsetDateTime::now_utc)
}

/// Construct a provider that always reports `instant`.
///
/// # Examples
///
/// ```rust
/// use netsuke::stdlib::{ClockProvider, fixed_clock};
/// use time::macros::datetime;
///
/// let instant = datetime!(2026-06-08 12:00:00 UTC);
/// let clock: ClockProvider = fixed_clock(instant);
/// assert_eq!(clock(), clock());
/// ```
#[must_use]
pub fn fixed_clock(instant: OffsetDateTime) -> ClockProvider {
    Arc::new(move || instant)
}

/// Wall-clock source held by `StdlibConfig` and captured at registration.
///
/// Named `WallClock` to keep it distinct from the monotonic-clock vocabulary
/// already in the crate: `monotony::MonotonicClock`, the `Clock` generic
/// parameter in `src/runner/process/mod.rs`, and the private
/// `type MonotonicClock` in `src/status_timing.rs`.
#[derive(Clone)]
pub(crate) struct WallClock {
    /// Provider consulted on every `now()` evaluation.
    provider: ClockProvider,
    /// Whether this is the ambient host clock, recorded for diagnostics.
    is_system: bool,
}

impl WallClock {
    /// Wrap `provider` as the clock backing `now()`.
    pub(crate) const fn new(provider: ClockProvider) -> Self {
        Self {
            provider,
            is_system: false,
        }
    }

    /// Read the current instant, normalized to UTC.
    ///
    /// Normalization is part of the contract, not a convenience: `now()` is
    /// documented to yield a UTC timestamp, and an injected provider is free
    /// to return any offset. Without this the harness could assert behaviour
    /// production never exhibits.
    pub(crate) fn read(&self) -> OffsetDateTime {
        (self.provider)().to_offset(UtcOffset::UTC)
    }

    /// Whether the ambient host clock is installed.
    pub(crate) const fn is_system(&self) -> bool {
        self.is_system
    }
}

impl Default for WallClock {
    fn default() -> Self {
        Self {
            provider: system_clock(),
            is_system: true,
        }
    }
}

/// Report the clock's provenance without pretending a closure is printable.
///
/// The label makes a wrongly wired clock self-diagnosing: an injected clock that
/// never reached registration, or an ambient clock where a test expected an
/// injected one, is visible in any `{:?}` of the surrounding configuration.
impl fmt::Debug for WallClock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WallClock")
            .field(
                "source",
                &if self.is_system() {
                    "system"
                } else {
                    "injected"
                },
            )
            .finish_non_exhaustive()
    }
}
