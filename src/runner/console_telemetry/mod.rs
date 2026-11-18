//! Record bounded console-scheduling telemetry around actual Ninja builds.
//!
//! This runner-only boundary accepts a graph aggregate and an invocation
//! closure. It emits no paths, identifiers, commands, or process error text.

use super::process::NinjaInvocationError;
use metrics::{counter, describe_counter, describe_histogram, histogram};
use monotony::MonotonicClock;
use std::{sync::Once, time::Duration};
use tracing::field;

/// Metric counting actual runner-owned Ninja build invocations.
pub const NINJA_BUILD_EXECUTIONS_TOTAL: &str = "netsuke_runner_ninja_build_executions_total";
/// Metric measuring actual runner-owned Ninja build duration in seconds.
pub const NINJA_BUILD_EXECUTION_DURATION: &str =
    "netsuke_runner_ninja_build_execution_duration_seconds";
/// Metric counting selected console edges across attempted Ninja build invocations.
pub const NINJA_BUILD_CONSOLE_EDGES_TOTAL: &str = "netsuke_runner_ninja_build_console_edges_total";

/// Return the fixed scheduling label for one aggregate console-edge count.
pub(in crate::runner) const fn scheduling_label(console_edge_count: usize) -> &'static str {
    if console_edge_count == 0 {
        "disabled"
    } else {
        "enabled"
    }
}

/// Record the outcome and duration of one actual Ninja build invocation.
///
/// Count configured user edges in the attempted build graph, rather than
/// inferring which edges Ninja actually executed or how long they occupied its pool.
///
/// The closure keeps the typed invocation error so the recorded category can
/// tell a failed launch from an unsuccessful exit and from any other process
/// failure; flattening it into an [`std::io::Error`] first would lose that
/// distinction.
///
/// # Errors
///
/// Returns the invocation's error unchanged when Ninja execution fails.
pub(in crate::runner) fn instrument_ninja_build<T>(
    clock: &dyn MonotonicClock,
    console_edge_count: usize,
    invoke: impl FnOnce() -> Result<T, NinjaInvocationError>,
) -> Result<T, NinjaInvocationError> {
    describe_metrics();
    let console_scheduling = scheduling_label(console_edge_count);
    let span = tracing::info_span!(
        "runner.ninja.build",
        console_edge_count,
        console_scheduling,
        outcome = field::Empty,
        error_category = field::Empty,
    );
    let _guard = span.enter();
    let started = clock.now();
    let result = invoke();
    record_result(
        &span,
        console_edge_count,
        clock.now().duration_since(started),
        &result,
    );
    result
}

/// Emit the bounded result, duration, and configured-edge aggregate.
fn record_result<T>(
    span: &tracing::Span,
    console_edge_count: usize,
    elapsed: Duration,
    result: &Result<T, NinjaInvocationError>,
) {
    let console_scheduling = scheduling_label(console_edge_count);
    let (outcome, error_category) = match result {
        Ok(_) => ("success", "none"),
        Err(error) => ("error", failure_category(error)),
    };
    span.record("outcome", outcome);
    span.record("error_category", error_category);
    tracing::info!(
        console_edge_count,
        console_scheduling,
        outcome,
        error_category,
        "Completed Ninja build invocation"
    );
    counter!(NINJA_BUILD_EXECUTIONS_TOTAL,
        "outcome" => outcome, "error_category" => error_category,
        "console_scheduling" => console_scheduling)
    .increment(1);
    histogram!(NINJA_BUILD_EXECUTION_DURATION,
        "outcome" => outcome, "error_category" => error_category,
        "console_scheduling" => console_scheduling)
    .record(elapsed);
    counter!(NINJA_BUILD_CONSOLE_EDGES_TOTAL, "outcome" => outcome)
        .increment(u64::try_from(console_edge_count).unwrap_or(u64::MAX));
}

/// Return the fixed telemetry category for one typed Ninja failure.
///
/// The three categories mirror the classification the runner uses to choose a
/// diagnostic, so a metric consumer can tell a launch failure from a recipe
/// that ran and failed without reading process error text.
const fn failure_category(error: &NinjaInvocationError) -> &'static str {
    match error {
        NinjaInvocationError::Spawn { .. } => "launch",
        NinjaInvocationError::Exit(_) => "exit_status",
        NinjaInvocationError::Io(_) => "io",
    }
}

/// Register the fixed Ninja-build metric descriptions once per process.
fn describe_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            NINJA_BUILD_EXECUTIONS_TOTAL,
            "Counts runner-owned Ninja builds by console scheduling, outcome, and error category."
        );
        describe_histogram!(
            NINJA_BUILD_EXECUTION_DURATION,
            "Measures Ninja build duration in seconds by console scheduling, outcome, and error category."
        );
        describe_counter!(
            NINJA_BUILD_CONSOLE_EDGES_TOTAL,
            concat!(
                "Counts configured console edges across attempted runner-owned builds; ",
                "it does not count executed edges."
            )
        );
    });
}

#[cfg(test)]
mod tests;
