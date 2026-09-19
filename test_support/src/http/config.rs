//! Timeout configuration for the local HTTP fixture.
//!
//! The fixture listener is non-blocking, so every wait it performs is a polling
//! loop with a deadline. This module owns the deadlines themselves and the
//! environment overrides that set them; the loops that consume them live in
//! `accept`, `request`, and `server`.

use std::{
    fmt,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

use mockable::{DefaultEnv, Env};

use super::accept::AcceptWait;

/// Override for the timeout in milliseconds within which a client must connect.
pub(crate) const ENV_HTTP_ACCEPT_TIMEOUT_MS: &str = "NETSUKE_TEST_HTTP_ACCEPT_TIMEOUT_MS";
/// Override for the timeout in milliseconds within which the request must arrive.
pub(crate) const ENV_HTTP_READ_TIMEOUT_MS: &str = "NETSUKE_TEST_HTTP_READ_TIMEOUT_MS";
/// Override for the polling interval in milliseconds used while waiting.
pub(crate) const ENV_HTTP_POLL_INTERVAL_MS: &str = "NETSUKE_TEST_HTTP_POLL_INTERVAL_MS";

#[cfg(test)]
use std::{cell::RefCell, thread_local};

#[cfg(test)]
thread_local! {
    /// Warnings recorded by this test thread's duration parses.
    static DURATION_WARNINGS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Configuration for HTTP fixtures, including timeouts used during polling.
#[derive(Debug, Clone)]
pub struct HttpServerConfig {
    /// Deadline for a client to connect.
    pub(super) accept_timeout: Duration,
    /// Deadline for the request to finish arriving.
    pub(super) read_timeout: Duration,
    /// Interval between readiness polls.
    pub(super) poll_interval: Duration,
    /// Whether the accept loop waits for a client without a deadline.
    ///
    /// Set by fixtures that expect no request: nothing but a shutdown ends
    /// their wait, so a deadline there would fail a slow machine rather than a
    /// wrong test.
    pub(super) accept_without_deadline: bool,
}

impl HttpServerConfig {
    /// Load configuration from environment variables, falling back to defaults.
    ///
    /// The following environment variables are honoured when present:
    ///
    /// * `NETSUKE_TEST_HTTP_ACCEPT_TIMEOUT_MS` – deadline for accepting a
    ///   connection in milliseconds.
    /// * `NETSUKE_TEST_HTTP_READ_TIMEOUT_MS` – deadline for reading the request
    ///   header block in milliseconds. The fixture answers once it has the
    ///   header block, so a request body is outside this deadline.
    /// * `NETSUKE_TEST_HTTP_POLL_INTERVAL_MS` – polling interval used when
    ///   waiting for readiness in milliseconds.
    ///
    /// Notes:
    /// Polling interval overrides are clamped to a minimum of 1 ms to avoid
    /// busy-spinning when the environment provides `0`.
    #[must_use]
    pub fn from_env() -> Self {
        Self::from_env_provider(&DefaultEnv)
    }

    /// Load the configuration from `env`, clamping the poll interval to 1 ms.
    pub(super) fn from_env_provider(env: &impl Env) -> Self {
        let mut config = Self::default();
        config.accept_timeout =
            duration_from_env(env, ENV_HTTP_ACCEPT_TIMEOUT_MS, config.accept_timeout);
        config.read_timeout = duration_from_env(env, ENV_HTTP_READ_TIMEOUT_MS, config.read_timeout);
        // Prevent busy-spin when overrides specify a zero-millisecond poll
        // interval. Tests only need millisecond precision, so clamp to at
        // least 1 ms.
        config.poll_interval =
            duration_from_env(env, ENV_HTTP_POLL_INTERVAL_MS, config.poll_interval)
                .max(Duration::from_millis(1));
        config
    }

    /// Return the instant by which a client must connect.
    fn accept_deadline(&self) -> Instant {
        Instant::now() + self.accept_timeout
    }

    /// Return a copy of this configuration that accepts without a deadline.
    #[must_use]
    pub(super) const fn accepting_until_shutdown(mut self) -> Self {
        self.accept_without_deadline = true;
        self
    }

    /// Return what the accept loop should wait for.
    ///
    /// An unbounded wait is given `shutdown` so the loop can stop on the
    /// signal alone, without depending on the wake-up connection a join sends.
    pub(super) fn accept_wait<'a>(&self, shutdown: &'a AtomicBool) -> AcceptWait<'a> {
        if self.accept_without_deadline {
            AcceptWait::UntilShutdown(shutdown)
        } else {
            AcceptWait::Until(self.accept_deadline())
        }
    }

    /// Return the instant by which the request must be read.
    pub(super) fn read_deadline(&self) -> Instant {
        Instant::now() + self.read_timeout
    }
}

impl Default for HttpServerConfig {
    fn default() -> Self {
        Self {
            accept_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(5),
            poll_interval: Duration::from_millis(10),
            accept_without_deadline: false,
        }
    }
}

/// Read `var` as whole milliseconds, falling back to `default` when unset or
/// unparsable.
///
/// An override is taken at face value, however large, and no check on the way to
/// a deadline is needed, because there is no value this can reject. A `u64`
/// millisecond count is at most about 1.8e16 seconds, and `Instant` on every
/// target this crate builds for carries at least a signed 64-bit seconds field
/// — roughly 9.2e18 — so `Instant::now() + Duration::from_millis(u64::MAX)`
/// neither overflows nor saturates. `checked_add` agrees: it returns `Some` for
/// `u64::MAX` milliseconds, so guarding the deadline with it would be dead
/// branch that no input can reach.
pub(super) fn duration_from_env(env: &impl Env, var: &str, default: Duration) -> Duration {
    env.raw(var).map_or(default, |value| {
        let trimmed = value.trim();
        match trimmed.parse::<u64>() {
            Ok(ms) => Duration::from_millis(ms),
            Err(err) => {
                log_duration_parse_error(var, trimmed.len(), &err);
                default
            }
        }
    })
}

/// Report an unparsable duration override without echoing its value.
///
/// The value is redacted: an environment variable's contents are outside this
/// crate's control, and logging them verbatim would put whatever the caller
/// exported into the log. `err` already names the bounded parse failure, and
/// `value_len` distinguishes an empty override from a malformed one, which is
/// all the diagnosis this fixture needs.
fn log_duration_parse_error(var: &str, value_len: usize, err: &dyn fmt::Display) {
    #[cfg(test)]
    {
        record_duration_warning(format!(
            "ignoring invalid {var}: {err} (value redacted, {value_len} bytes)"
        ));
    }

    #[cfg(not(test))]
    {
        tracing::warn!(
            variable = var,
            value_len,
            error = %err,
            "ignoring invalid fixture duration"
        );
    }
}

#[cfg(test)]
fn record_duration_warning(message: String) {
    DURATION_WARNINGS.with(|warnings| warnings.borrow_mut().push(message));
}

/// Drain the duration-parse warnings recorded on this thread.
#[cfg(test)]
pub(super) fn take_duration_warnings() -> Vec<String> {
    DURATION_WARNINGS.with(|warnings| warnings.borrow_mut().drain(..).collect())
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod config_tests;
