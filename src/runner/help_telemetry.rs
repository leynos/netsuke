//! Bounded observability for the `netsuke help targets` query boundary.
//!
//! The catalogue consumes manifest-controlled names and descriptions, so this
//! module records only fixed operation outcomes and error categories.

use anyhow::Result;
use metrics::{counter, describe_counter, describe_histogram, histogram};
use std::{sync::Once, time::Instant};
use tracing::{field, info};

use super::super::RunnerError;

pub(super) const HELP_TARGETS_TOTAL: &str = "netsuke_runner_help_targets_total";
pub(super) const HELP_TARGETS_DURATION: &str = "netsuke_runner_help_targets_duration_seconds";

/// Record bounded telemetry around the complete `help targets` query.
pub(super) fn instrument_help_targets<T>(query: impl FnOnce() -> Result<T>) -> Result<T> {
    describe_help_targets_metrics();
    let span = tracing::info_span!(
        "runner.help_targets",
        outcome = field::Empty,
        error_category = field::Empty,
    );
    let _guard = span.enter();
    let started = Instant::now();
    let result = query();
    let (outcome, error_category) = match &result {
        Ok(_) => ("success", "none"),
        Err(error) => ("error", help_targets_error_category(error)),
    };
    span.record("outcome", outcome);
    span.record("error_category", error_category);
    info!(outcome, error_category, "Completed help targets query");
    counter!(
        HELP_TARGETS_TOTAL,
        "outcome" => outcome,
        "error_category" => error_category,
    )
    .increment(1);
    histogram!(
        HELP_TARGETS_DURATION,
        "outcome" => outcome,
        "error_category" => error_category,
    )
    .record(started.elapsed());
    result
}

/// Classify catalogue failures without exposing manifest-controlled detail.
fn help_targets_error_category(error: &anyhow::Error) -> &'static str {
    if error.downcast_ref::<RunnerError>().is_some() {
        "manifest_not_found"
    } else {
        "other"
    }
}

/// Describe the stable, bounded help-targets metrics once per process.
fn describe_help_targets_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            HELP_TARGETS_TOTAL,
            "Counts help-target catalogue queries by bounded outcome and error category."
        );
        describe_histogram!(
            HELP_TARGETS_DURATION,
            "Measures complete help-target catalogue query duration in seconds by bounded outcome and error category."
        );
    });
}
