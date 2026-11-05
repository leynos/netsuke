//! Telemetry coverage for the manifest schema-admission gate.
//!
//! These drive the real loader under a local recorder, so they pin that every
//! admission decision — the successful one and each rejection category —
//! reaches the bounded counter exactly once, that the accompanying span and
//! event carry only the fixed category, and that nothing a manifest supplies
//! reaches a label or a trace field.

use super::super::from_str;
use crate::test_tracing_capture::with_test_subscriber;
use anyhow::{Context, Result, ensure};
use metrics::SharedString;
use metrics_util::{
    CompositeKey, MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use rstest::rstest;
use tracing::level_filters::LevelFilter;

/// The counter this module's tests read back from the local recorder.
const SCHEMA_ADMISSION_TOTAL: &str = "netsuke_manifest_schema_admissions_total";

/// Stands in for a private manifest path or a declared version string; neither
/// may reach a metric label or a captured trace field.
const SENTINEL: &str = "sentinel-version-9.9.9";

type Snapshot = Vec<(
    CompositeKey,
    Option<metrics::Unit>,
    Option<SharedString>,
    DebugValue,
)>;

/// One manifest body per rejection category, plus the admitted control.
///
/// The bodies are complete manifests rather than bare fragments so the loader
/// reaches the admission gate down the same path a real manifest does.
fn manifest_body(outcome: &str) -> String {
    let declaration = match outcome {
        "admitted" => "netsuke_version: '1.0.0'\n",
        "missing" => "",
        "not_string" => "netsuke_version: 7\n",
        "invalid" => "netsuke_version: 'not-semver'\n",
        "unsupported" => "netsuke_version: '1.1.0'\n",
        "not_canonical" => "netsuke_version: '1.0.1'\n",
        other => unreachable!("no fixture for outcome {other}"),
    };
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
fn recorded(yaml: &str) -> (Result<(), anyhow::Error>, Snapshot) {
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

/// Every schema-admission series must carry exactly the one bounded `outcome`
/// label.
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
#[case::admitted("admitted", true)]
#[case::missing("missing", false)]
#[case::not_string("not_string", false)]
#[case::invalid("invalid", false)]
#[case::unsupported("unsupported", false)]
#[case::not_canonical("not_canonical", false)]
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

/// The decision point emits one trace event carrying the fixed category only.
///
/// The declared version carries [`SENTINEL`] in its build metadata, so a
/// telemetry field built from the version text rather than its category would
/// be observable. The version is still unsupported, because admission compares
/// only the major and minor components.
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
