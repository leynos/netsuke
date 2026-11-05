//! Bounded telemetry for the manifest schema-admission gate.
//!
//! Every loaded document passes exactly one admission decision, and every
//! decision reaches [`record_admission`], so the gate has a single telemetry
//! point rather than one per call site. The counter is the operational signal
//! for the gate: a rejection is otherwise visible only in the diagnostic a
//! person reads, which is neither aggregated nor retained.
//!
//! What is recorded is deliberately bounded. The only label is `outcome`,
//! drawn from the closed vocabulary below, so the series count is fixed by this
//! module rather than by anything a manifest supplies. The manifest path and the
//! declared version are absent by construction: a path can identify a private
//! project, and a raw version string is caller-controlled text.
//!
//! The predicate itself stays pure. [`super::schema_version`] decides; this
//! module only observes the decision, so the admission rule remains testable
//! without a metrics recorder.

use super::schema_version::SchemaVersionRejection;
use metrics::{counter, describe_counter};
use std::sync::Once;
use tracing::{trace, trace_span};

/// Counts manifest schema-admission decisions by bounded `outcome`.
pub const SCHEMA_ADMISSION_TOTAL: &str = "netsuke_manifest_schema_admissions_total";

/// The bounded `outcome` recorded when the declaration is admitted.
pub(super) const OUTCOME_ADMITTED: &str = "admitted";
/// The bounded `outcome` recorded when the declaration is absent.
pub(super) const OUTCOME_MISSING: &str = "missing";
/// The bounded `outcome` recorded when the declaration is not a string.
pub(super) const OUTCOME_NOT_STRING: &str = "not_string";
/// The bounded `outcome` recorded when the declaration is not a semantic
/// version.
pub(super) const OUTCOME_INVALID: &str = "invalid";
/// The bounded `outcome` recorded when the declaration is outside the
/// supported range.
pub(super) const OUTCOME_UNSUPPORTED: &str = "unsupported";
/// The bounded `outcome` recorded when the declaration is inside the supported
/// range but is not a schema release.
pub(super) const OUTCOME_NOT_CANONICAL: &str = "not_canonical";

/// The closed `outcome` vocabulary admitted on [`SCHEMA_ADMISSION_TOTAL`].
pub const SCHEMA_ADMISSION_OUTCOME_VALUES: [&str; 6] = [
    OUTCOME_ADMITTED,
    OUTCOME_MISSING,
    OUTCOME_NOT_STRING,
    OUTCOME_INVALID,
    OUTCOME_UNSUPPORTED,
    OUTCOME_NOT_CANONICAL,
];

/// Describe the schema-admission counter once per process.
fn describe_schema_admission_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            SCHEMA_ADMISSION_TOTAL,
            "Counts manifest schema-admission decisions labelled by outcome: \
             admitted, or the rejection category missing, not_string, invalid, \
             unsupported, or not_canonical."
        );
    });
}

/// Record one admission decision and return it unchanged.
///
/// This is the telemetry boundary for the gate: every decision that reaches it
/// is counted exactly once, whatever the outcome. `outcome` is always one of
/// the constants above — a literal never reaches the label — so the series stay
/// bounded and neither the manifest path nor the declared version can appear.
pub(super) fn record_admission(
    decision: Result<(), SchemaVersionRejection>,
) -> Result<(), SchemaVersionRejection> {
    describe_schema_admission_metrics();
    let outcome = match &decision {
        Ok(()) => OUTCOME_ADMITTED,
        Err(rejection) => outcome_label(*rejection),
    };
    debug_assert!(
        SCHEMA_ADMISSION_OUTCOME_VALUES.contains(&outcome),
        "schema-admission telemetry must use a closed outcome vocabulary"
    );
    let span = trace_span!(
        "manifest.schema.admission",
        operation = "admit_schema",
        outcome
    );
    let _guard = span.enter();
    counter!(SCHEMA_ADMISSION_TOTAL, "outcome" => outcome).increment(1);
    trace!(outcome, "manifest schema admission decision");
    decision
}

/// Map a typed rejection to its fixed telemetry label.
///
/// The rejection carries the declared major and minor so the diagnostic can
/// name them, but the label names only the category: a version string is
/// caller-controlled text and must not become a metric dimension.
const fn outcome_label(rejection: SchemaVersionRejection) -> &'static str {
    match rejection {
        SchemaVersionRejection::Missing => OUTCOME_MISSING,
        SchemaVersionRejection::NotAString => OUTCOME_NOT_STRING,
        SchemaVersionRejection::Invalid => OUTCOME_INVALID,
        SchemaVersionRejection::Unsupported { .. } => OUTCOME_UNSUPPORTED,
        SchemaVersionRejection::NotCanonical => OUTCOME_NOT_CANONICAL,
    }
}
