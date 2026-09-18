//! Bounded telemetry for the manifest `env()` lookup boundary.
//!
//! Every `env()` call reaches exactly one place: [`super::env_reader::env_var_with`]
//! evaluates the access policy, reads through the injected reader, and maps
//! failures to Jinja errors. That single boundary is therefore also the single
//! telemetry point, and each lookup is counted once under a closed `outcome`
//! vocabulary.
//!
//! The blocked outcome is the reason this series exists. Denying a lookup is
//! new behaviour that previously could not occur, so without a counter an
//! operator has no way to measure how often a policy refuses manifest access;
//! the `tracing` event that accompanies the refusal is not aggregated and is
//! not retained by the application recorder.
//!
//! What is recorded is deliberately bounded and redacted. The only label is
//! `outcome`, drawn from the constant set below, so the number of series is
//! fixed by this module rather than by anything a manifest supplies. The
//! variable name and its value are absent by construction: environment
//! variable names routinely identify credentials, and a rendered value can
//! carry secret material.

use metrics::{counter, describe_counter};
use minijinja::Error;
use std::sync::Once;

/// Counts manifest `env()` lookups by bounded `outcome`.
///
/// The label is drawn from the closed set below, so the series count is fixed
/// by this module rather than by anything a manifest supplies. The application
/// recorder admits the series through the same set, so the counter is exported
/// rather than silently dropped as a noop handle.
pub const ENV_LOOKUP_TOTAL: &str = "netsuke_manifest_env_lookups_total";

/// The bounded `outcome` recorded when the lookup produced a value.
pub(super) const OUTCOME_SUCCESS: &str = "success";
/// The bounded `outcome` recorded when the access policy denied the name.
pub(super) const OUTCOME_BLOCKED: &str = "blocked";
/// The bounded `outcome` recorded when the variable is absent.
pub(super) const OUTCOME_NOT_PRESENT: &str = "not_present";
/// The bounded `outcome` recorded when the value is not valid UTF-8.
pub(super) const OUTCOME_NOT_UNICODE: &str = "not_unicode";

/// The closed `outcome` vocabulary admitted on [`ENV_LOOKUP_TOTAL`].
pub const ENV_LOOKUP_OUTCOME_VALUES: [&str; 4] = [
    OUTCOME_SUCCESS,
    OUTCOME_BLOCKED,
    OUTCOME_NOT_PRESENT,
    OUTCOME_NOT_UNICODE,
];

/// Describe the environment-lookup counter once per process.
fn describe_env_lookup_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            ENV_LOOKUP_TOTAL,
            "Counts manifest env() lookups labelled by outcome: success, \
             blocked when the access policy denied the name, not_present \
             when the variable is absent, or not_unicode when its value is \
             not valid UTF-8."
        );
    });
}

/// Record one environment lookup and return its result unchanged.
///
/// This is the telemetry boundary for the `env()` helper: every lookup that
/// reaches it is counted exactly once, whatever the outcome. `outcome` is
/// always one of the constants above — a literal never reaches the label — so
/// the series stay bounded and the variable name and value stay out of both
/// the metric and any subscriber.
pub(super) fn record_env_lookup<T>(
    outcome: &'static str,
    result: Result<T, Error>,
) -> Result<T, Error> {
    describe_env_lookup_metrics();
    debug_assert!(
        ENV_LOOKUP_OUTCOME_VALUES.contains(&outcome),
        "env lookup telemetry must use a closed outcome vocabulary"
    );
    counter!(ENV_LOOKUP_TOTAL, "outcome" => outcome).increment(1);
    result
}
