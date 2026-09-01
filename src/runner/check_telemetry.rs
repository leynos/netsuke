//! Bounded observability for the `netsuke check` command boundary.
//!
//! The linter consumes manifest text and produces rule messages, neither of
//! which may become metric labels. This adapter therefore records only the
//! command outcome and complete command duration.

use anyhow::Result;
use metrics::{counter, describe_counter, describe_histogram, histogram};
use std::{sync::Once, time::Instant};
use tracing::{field, info};

/// Metric name counting complete check invocations by fixed outcome.
pub const CHECK_TOTAL: &str = "netsuke_runner_check_total";
/// Metric name measuring complete check invocation duration in seconds.
pub const CHECK_DURATION: &str = "netsuke_runner_check_duration_seconds";

/// A command failure classified before it reaches the generic error boundary.
pub(super) enum CheckFailure {
    /// A selector or threshold was invalid.
    Policy(anyhow::Error),
    /// Loading, lowering, or lint analysis failed.
    Analysis(anyhow::Error),
    /// Writing a result, diagnostic, or rule reference failed.
    Output(anyhow::Error),
    /// Findings reached the configured threshold.
    Threshold(anyhow::Error),
}

impl CheckFailure {
    /// Name the bounded outcome represented by this failure.
    const fn outcome(&self) -> &'static str {
        match self {
            Self::Policy(_) => "policy_failure",
            Self::Analysis(_) => "analysis_failure",
            Self::Output(_) => "output_failure",
            Self::Threshold(_) => "threshold_failure",
        }
    }

    /// Recover the original diagnostic for the application's error renderer.
    fn into_inner(self) -> anyhow::Error {
        match self {
            Self::Policy(error)
            | Self::Analysis(error)
            | Self::Output(error)
            | Self::Threshold(error) => error,
        }
    }
}

/// Instrument one complete check command with fixed outcome telemetry.
pub(super) fn instrument_check(check: impl FnOnce() -> Result<(), CheckFailure>) -> Result<()> {
    describe_metrics();
    let span = tracing::info_span!("runner.check", outcome = field::Empty);
    let _guard = span.enter();
    let started = Instant::now();
    let result = check();
    let outcome = result
        .as_ref()
        .map_or_else(CheckFailure::outcome, |()| "success");
    span.record("outcome", outcome);
    info!(outcome, "Completed manifest check");
    counter!(CHECK_TOTAL, "outcome" => outcome).increment(1);
    histogram!(CHECK_DURATION, "outcome" => outcome).record(started.elapsed());
    result.map_err(CheckFailure::into_inner)
}

/// Register the check metric descriptions once per process.
fn describe_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            CHECK_TOTAL,
            "Counts `netsuke check` commands by fixed outcome."
        );
        describe_histogram!(
            CHECK_DURATION,
            "Measures complete `netsuke check` duration in seconds by fixed outcome."
        );
    });
}

#[cfg(test)]
#[path = "check_telemetry_tests.rs"]
mod tests;
