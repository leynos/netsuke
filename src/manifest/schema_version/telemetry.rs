//! Bounded telemetry for the manifest schema-admission gate.
//!
//! Every full load passes exactly one admission decision, and every decision
//! that records reaches [`record_admission`], so the gate has a single
//! telemetry point rather than one per call site. Recording is gated on
//! `StdlibRegistration::records_schema_admission`, which is true for
//! `AmbientFull` and `Full` only: discovery queries and `netsuke test` runs
//! reach the same gate but are not admissions of a user's manifest, so they
//! emit neither a counter increment nor a decision event. The counter is the
//! operational signal for the gate: a rejection is otherwise visible only in
//! the diagnostic a person reads, which is neither aggregated nor retained.
//!
//! What is recorded is deliberately bounded. The only label is `outcome`,
//! drawn from the closed vocabulary below, so the series count is fixed by this
//! module rather than by anything a manifest supplies. The manifest path and the
//! declared version are absent by construction: a path can identify a private
//! project, and a raw version string is caller-controlled text.
//!
//! The predicate itself stays pure. [`super`] decides; this module only
//! observes the decision, so the admission rule remains testable without a
//! metrics recorder.

use super::SchemaVersionRejection;
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
///
/// Only a load whose registration records admission reaches this function;
/// `StdlibRegistration::records_schema_admission` is the switch, and it is off
/// for discovery queries and `netsuke test` runs. What it admits is a full
/// load, so a direct `netsuke::manifest::from_str` call is counted as well as a
/// Netsuke command. The counter therefore measures full-load admissions, not
/// load attempts.
pub(in crate::manifest) fn record_admission(
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

#[cfg(test)]
mod tests {
    //! Drive the real loader under a local recorder.
    //!
    //! These tests pin that every admission decision — the successful one and
    //! each rejection category — reaches the bounded counter exactly once, that
    //! the accompanying span and event carry only the fixed category, and that
    //! nothing a manifest supplies reaches a label or a trace field.

    use super::{
        OUTCOME_ADMITTED, OUTCOME_INVALID, OUTCOME_MISSING, OUTCOME_NOT_CANONICAL,
        OUTCOME_NOT_STRING, OUTCOME_UNSUPPORTED, SCHEMA_ADMISSION_TOTAL,
    };
    use crate::manifest::from_str;
    use crate::test_tracing_capture::with_test_subscriber;
    use anyhow::{Context, Result, ensure};
    use metrics::SharedString;
    use metrics_util::{
        CompositeKey, MetricKind,
        debugging::{DebugValue, DebuggingRecorder},
    };
    use rstest::rstest;
    use tracing::level_filters::LevelFilter;

    /// Stands in for a private manifest path or a declared version string;
    /// neither may reach a metric label or a captured trace field.
    const SENTINEL: &str = "sentinel-version-9.9.9";

    type Snapshot = Vec<(
        CompositeKey,
        Option<metrics::Unit>,
        Option<SharedString>,
        DebugValue,
    )>;

    /// One manifest body per rejection category, plus the admitted control.
    ///
    /// The bodies are complete manifests rather than bare fragments so the
    /// loader reaches the admission gate down the same path a real manifest
    /// does. Pairing each body with its outcome keeps the closed vocabulary and
    /// the fixture set from drifting apart: the case table below names every
    /// entry here, so a category added to one without the other fails a test.
    const FIXTURES: [(&str, &str); 6] = [
        (OUTCOME_ADMITTED, "netsuke_version: '1.0.0'\n"),
        (OUTCOME_MISSING, ""),
        (OUTCOME_NOT_STRING, "netsuke_version: 7\n"),
        (OUTCOME_INVALID, "netsuke_version: 'not-semver'\n"),
        (OUTCOME_UNSUPPORTED, "netsuke_version: '1.1.0'\n"),
        (OUTCOME_NOT_CANONICAL, "netsuke_version: '1.0.1'\n"),
    ];

    /// Build the manifest body declared for `outcome`.
    ///
    /// An outcome with no fixture yields an empty declaration, which the gate
    /// reports as [`OUTCOME_MISSING`] rather than panicking. No test uses an
    /// outcome outside [`FIXTURES`], so the fallback only keeps this helper
    /// total.
    fn manifest_body(outcome: &str) -> String {
        let declaration = FIXTURES
            .iter()
            .find_map(|(name, body)| (*name == outcome).then_some(*body))
            .unwrap_or_default();
        format!(
            concat!(
                "{declaration}",
                "targets:\n",
                "  - name: hello\n",
                "    command: echo hello\n",
            ),
            declaration = declaration
        )
    }

    /// Drive one load under a local recorder and return its result and snapshot.
    fn recorded(yaml: &str) -> (Result<()>, Snapshot) {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        let result = metrics::with_local_recorder(&recorder, || from_str(yaml).map(drop));
        (result, snapshotter.snapshot().into_vec())
    }

    /// Value of the schema-admission counter labelled with `outcome`.
    fn admission_count(snapshot: &Snapshot, outcome: &str) -> Option<u64> {
        snapshot.iter().find_map(|(key, _, _, value)| {
            if key.kind() != MetricKind::Counter || key.key().name() != SCHEMA_ADMISSION_TOTAL {
                return None;
            }
            let has_outcome = key
                .key()
                .labels()
                .any(|label| label.key() == "outcome" && label.value() == outcome);
            match value {
                DebugValue::Counter(count) if has_outcome => Some(*count),
                _ => None,
            }
        })
    }

    /// Every schema-admission series must carry exactly the one bounded
    /// `outcome` label.
    ///
    /// The assertion is scoped to this counter: an admitted manifest goes on to
    /// load, expand, and render, so the same recorder legitimately observes the
    /// filtering and render series those stages emit.
    fn every_admission_series_is_bounded(snapshot: &Snapshot) -> bool {
        snapshot
            .iter()
            .filter(|(key, _, _, _)| key.key().name() == SCHEMA_ADMISSION_TOTAL)
            .all(|(key, _, _, _)| {
                key.key().labels().count() == 1
                    && key.key().labels().all(|label| label.key() == "outcome")
            })
    }

    /// Number of retained series belonging to the schema-admission counter.
    fn admission_series_count(snapshot: &Snapshot) -> usize {
        snapshot
            .iter()
            .filter(|(key, _, _, _)| key.key().name() == SCHEMA_ADMISSION_TOTAL)
            .count()
    }

    #[rstest]
    #[case::admitted(OUTCOME_ADMITTED, true)]
    #[case::missing(OUTCOME_MISSING, false)]
    #[case::not_string(OUTCOME_NOT_STRING, false)]
    #[case::invalid(OUTCOME_INVALID, false)]
    #[case::unsupported(OUTCOME_UNSUPPORTED, false)]
    #[case::not_canonical(OUTCOME_NOT_CANONICAL, false)]
    fn each_admission_outcome_is_counted_once(
        #[case] expected_outcome: &str,
        #[case] is_admitted: bool,
    ) -> Result<()> {
        let (result, snapshot) = recorded(&manifest_body(expected_outcome));

        ensure!(
            result.is_ok() == is_admitted,
            "the {expected_outcome} fixture must {} load: {result:?}",
            if is_admitted { "" } else { "not" }
        );
        ensure!(
            admission_count(&snapshot, expected_outcome) == Some(1),
            "the decision must be counted under {expected_outcome}: {snapshot:?}"
        );
        ensure!(
            admission_series_count(&snapshot) == 1,
            "a single load must count one admission series: {snapshot:?}"
        );
        ensure!(
            every_admission_series_is_bounded(&snapshot),
            "the retained series must carry only the bounded outcome label: {snapshot:?}"
        );
        Ok(())
    }

    /// The decision point emits one trace event carrying the fixed category
    /// only.
    ///
    /// The declared version carries [`SENTINEL`] in its build metadata, so a
    /// telemetry field built from the version text rather than its category
    /// would be observable. The version is still unsupported, because admission
    /// compares only the major and minor components.
    #[test]
    fn admission_telemetry_carries_only_the_bounded_category() -> Result<()> {
        let yaml = format!(
            concat!(
                "netsuke_version: '1.1.0+{sentinel}'\n",
                "targets:\n",
                "  - name: hello\n",
                "    command: echo hello\n",
            ),
            sentinel = SENTINEL
        );
        let recorder = DebuggingRecorder::new();
        let (events, span_fields) = metrics::with_local_recorder(&recorder, || {
            with_test_subscriber(LevelFilter::TRACE, |captured| {
                from_str(&yaml).expect_err("an unsupported schema must be rejected");
                (
                    captured.snapshot(),
                    captured.span_fields("manifest.schema.admission"),
                )
            })
        });

        let decision = events
            .iter()
            .find(|event| event.contains("manifest schema admission decision"))
            .context("the gate must emit its decision event")?;
        ensure!(
            decision.contains("outcome=\"unsupported\""),
            "the event must name the bounded category: {decision}"
        );
        ensure!(
            span_fields
                .iter()
                .any(|field| field == "outcome=\"unsupported\""),
            "the span must name the bounded category: {span_fields:?}"
        );
        ensure!(
            events
                .iter()
                .chain(span_fields.iter())
                .all(|rendered| !rendered.contains(SENTINEL)),
            "the declared version text must not reach telemetry: {events:?} {span_fields:?}"
        );
        Ok(())
    }
}
