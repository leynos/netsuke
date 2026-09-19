//! Environment overrides for the fixture's timeouts.
//!
//! The fixture's deadlines are tunable so a slow machine can be given longer
//! than a default that suits a fast one. An override arrives as a string, so
//! this module owns the parsing, the fallback when it does not parse, and the
//! redaction that keeps a caller-supplied value out of the log.

use std::{fmt, time::Duration};

use mockable::Env;

#[cfg(test)]
use std::{cell::RefCell, thread_local};

#[cfg(test)]
thread_local! {
    /// Warnings recorded for assertions by the fixture's own tests.
    ///
    /// A `tracing` subscriber would be the production path, but a unit test
    /// cannot observe one without a global subscriber, so the test build
    /// captures into a thread-local instead.
    pub(super) static DURATION_WARNINGS: RefCell<Vec<String>> =
        const { RefCell::new(Vec::new()) };
}

/// Read `var` as whole milliseconds, falling back to `default` when unset or
/// unparsable.
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

/// Record a warning for the fixture's own tests to assert on.
#[cfg(test)]
fn record_duration_warning(message: String) {
    DURATION_WARNINGS.with(|warnings| warnings.borrow_mut().push(message));
}

/// Take the warnings recorded so far on this thread, leaving the buffer empty.
#[cfg(test)]
pub(super) fn take_duration_warnings() -> Vec<String> {
    DURATION_WARNINGS.with(|warnings| warnings.borrow_mut().drain(..).collect())
}
