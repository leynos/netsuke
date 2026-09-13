//! Bounded metrics for the fetch network boundary.
//!
//! Metric names and label vocabularies are declared in one place so the
//! bounding is auditable and the application-owned recorder can be written
//! against a fixed set of series. Every label value comes from one of the
//! closed lists below; no series carries a URL, host, location, or userinfo.
//!
//! The library only emits these series; the application boundary installs the
//! recorder, as ADR-013 requires.

use std::{sync::Once, time::Duration};

use metrics::{counter, describe_counter, describe_histogram, histogram};

/// Counter of completed `fetch` calls, labelled by `outcome`.
pub(super) const FETCH_TOTAL: &str = "netsuke_stdlib_fetch_total";
/// Histogram of `fetch` call durations in seconds.
pub(super) const FETCH_DURATION: &str = "netsuke_stdlib_fetch_duration_seconds";
/// Counter of network-policy decisions, labelled by `outcome` and `policy_reason`.
pub(super) const FETCH_POLICY_TOTAL: &str = "netsuke_stdlib_fetch_policy_total";
/// Counter of redirect decisions, labelled by `outcome` and `redirect_failure`.
pub(super) const FETCH_REDIRECT_TOTAL: &str = "netsuke_stdlib_fetch_redirect_total";

/// Outcomes admitted by [`FETCH_TOTAL`].
pub(super) const FETCH_OUTCOME_VALUES: [&str; 2] = ["success", "failure"];
/// Outcomes admitted by [`FETCH_POLICY_TOTAL`].
pub(super) const FETCH_POLICY_OUTCOME_VALUES: [&str; 2] = ["allowed", "rejected"];
/// Reasons admitted by [`FETCH_POLICY_TOTAL`], including the allowed outcome.
pub(super) const FETCH_POLICY_REASON_VALUES: [&str; 5] = [
    "allowed",
    "scheme_not_allowed",
    "missing_host",
    "host_not_allowlisted",
    "host_blocked",
];
/// Outcomes admitted by [`FETCH_REDIRECT_TOTAL`].
pub(super) const FETCH_REDIRECT_OUTCOME_VALUES: [&str; 2] = ["followed", "rejected"];
/// Failure categories admitted by [`FETCH_REDIRECT_TOTAL`], including none.
pub(super) const FETCH_REDIRECT_FAILURE_VALUES: [&str; 7] = [
    "none",
    "limit_exceeded",
    "loop",
    "location_missing",
    "location_invalid",
    "credentials_not_removable",
    "policy_rejected",
];

/// Record one completed fetch call with its duration and bounded outcome.
///
/// Describes the fetch series once per process, so the first recorded call also
/// registers the metadata an operator sees with the sample.
pub(super) fn record_fetch(duration: Duration, succeeded: bool) {
    describe_metrics();
    let outcome = if succeeded { "success" } else { "failure" };
    debug_assert!(
        FETCH_OUTCOME_VALUES.contains(&outcome),
        "a fetch outcome must come from the declared vocabulary",
    );
    histogram!(FETCH_DURATION).record(duration);
    counter!(FETCH_TOTAL, "outcome" => outcome).increment(1);
}

/// Record one network-policy decision with its bounded outcome and reason.
///
/// A label outside the declared vocabularies is a programming error and
/// panics in debug builds rather than silently widening the series.
pub(super) fn record_policy_decision(outcome: &'static str, reason: &'static str) {
    describe_metrics();
    debug_assert!(
        FETCH_POLICY_OUTCOME_VALUES.contains(&outcome),
        "a policy outcome must come from the declared vocabulary",
    );
    debug_assert!(
        FETCH_POLICY_REASON_VALUES.contains(&reason),
        "a policy reason must come from the declared vocabulary",
    );
    counter!(
        FETCH_POLICY_TOTAL,
        "outcome" => outcome,
        "policy_reason" => reason,
    )
    .increment(1);
}

/// Record one redirect the chain followed.
pub(super) fn record_redirect_followed() {
    describe_metrics();
    debug_assert!(
        FETCH_REDIRECT_OUTCOME_VALUES.contains(&"followed"),
        "a followed redirect must use the declared outcome vocabulary",
    );
    counter!(
        FETCH_REDIRECT_TOTAL,
        "outcome" => "followed",
        "redirect_failure" => "none",
    )
    .increment(1);
}

/// Record one redirect the chain refused, by bounded failure category.
///
/// A category outside [`FETCH_REDIRECT_FAILURE_VALUES`] is a programming error
/// and panics in debug builds rather than silently widening the series.
pub(super) fn record_redirect_refused(failure: &'static str) {
    describe_metrics();
    debug_assert!(
        FETCH_REDIRECT_OUTCOME_VALUES.contains(&"rejected"),
        "a refused redirect must use the declared outcome vocabulary",
    );
    debug_assert!(
        FETCH_REDIRECT_FAILURE_VALUES.contains(&failure),
        "a redirect failure must come from the declared vocabulary",
    );
    counter!(
        FETCH_REDIRECT_TOTAL,
        "outcome" => "rejected",
        "redirect_failure" => failure,
    )
    .increment(1);
}

/// Describe every fetch series once per process.
fn describe_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            FETCH_TOTAL,
            "Counts fetch calls labelled by success or failure."
        );
        describe_histogram!(
            FETCH_DURATION,
            "Measures fetch call duration in seconds."
        );
        describe_counter!(
            FETCH_POLICY_TOTAL,
            "Counts network-policy decisions labelled by allowed or rejected outcome and reason."
        );
        describe_counter!(
            FETCH_REDIRECT_TOTAL,
            "Counts redirect decisions labelled by followed or rejected outcome and failure category."
        );
    });
}

#[cfg(test)]
#[path = "telemetry_tests.rs"]
mod tests;
