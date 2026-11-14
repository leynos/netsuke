//! Telemetry coverage for the manifest schema-admission gate.
//!
//! The admission counter answers "how often is a real manifest admitted or
//! refused", so it must count command loads and stay silent for discovery
//! queries and `netsuke test` runs. Those three paths share one loader, which
//! is why the distinction is asserted here at the counter rather than inferred
//! from the registration table alone.

use super::super::{
    ManifestBudgetLimits, ManifestLoadOptions, ManifestName, TestSurface,
    from_path_for_manifest_query, from_str_with_limits, from_str_with_options,
};
use crate::stdlib::StdlibConfig;
use crate::test_tracing_capture::with_test_subscriber;
use anyhow::{Context, Result as AnyResult, anyhow, ensure};
use metrics::SharedString;
use metrics_util::{
    CompositeKey, MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use rstest::rstest;
use tempfile::tempdir;
use test_support::fs as test_fs;
use tracing_subscriber::filter::LevelFilter;

const ADMISSION_TOTAL: &str = "netsuke_manifest_schema_admissions_total";

/// The span and event `record_admission` emits beside the counter.
///
/// Recording is two effects in one place: a counter increment and a trace
/// event. Gating only the counter would leave the admission path observable
/// through the event, so both are asserted.
const ADMISSION_SPAN: &str = "manifest.schema.admission";

/// The message the decision event carries.
///
/// The event is a distinct surface from the span: a query that emitted only the
/// event would still be observable, so both are asserted separately.
const ADMISSION_EVENT: &str = "manifest schema admission decision";

/// One recorder snapshot as the debugging utilities expose it.
type Snapshot = Vec<(
    CompositeKey,
    Option<metrics::Unit>,
    Option<SharedString>,
    DebugValue,
)>;

/// Collect the `outcome` label and count of every retained admission series.
///
/// The assertion is scoped to this counter: an admitted manifest goes on to
/// expand and render, so the same recorder legitimately observes the series
/// those stages emit.
fn admission_counts(snapshot: &Snapshot) -> Vec<(String, u64)> {
    snapshot
        .iter()
        .filter(|(key, _, _, _)| {
            key.kind() == MetricKind::Counter && key.key().name() == ADMISSION_TOTAL
        })
        .filter_map(|(key, _, _, value)| {
            let outcome = key
                .key()
                .labels()
                .find(|label| label.key() == "outcome")?
                .value()
                .to_owned();
            match value {
                DebugValue::Counter(count) => Some((outcome, *count)),
                _ => None,
            }
        })
        .collect()
}

/// Write a manifest declaring `version` into a fresh workspace.
fn workspace_with(version: &str) -> AnyResult<(tempfile::TempDir, camino::Utf8PathBuf)> {
    let temp = tempdir().context("create schema admission workspace")?;
    let manifest_path = temp.path().join("Netsukefile");
    test_fs::write(&manifest_path, command_load_manifest(version))?;
    let utf8 = camino::Utf8PathBuf::from_path_buf(manifest_path)
        .map_err(|rejected| anyhow!("workspace path is not UTF-8: {}", rejected.display()))?;
    Ok((temp, utf8))
}

/// A discovery query must not move the admission counter.
///
/// The query still validates the declaration, so a rejected schema is still
/// reported; only the counting is withheld. Asserting on an admitted manifest
/// keeps the test honest: an absent series cannot be explained by an early
/// rejection returning first.
#[test]
fn manifest_query_does_not_record_schema_admission() -> AnyResult<()> {
    let (_temp, path) = workspace_with("1.0.0")?;
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || {
        from_path_for_manifest_query(&path, None)?;
        Ok::<_, anyhow::Error>(())
    })?;
    let counts = admission_counts(&snapshotter.snapshot().into_vec());
    ensure!(
        counts.is_empty(),
        "a manifest query must not record a schema admission: {counts:?}"
    );
    Ok(())
}

/// Write a command-load manifest declaring `version` as its schema.
fn command_load_manifest(version: &str) -> String {
    format!(
        concat!(
            "netsuke_version: \"{}\"\n",
            "targets:\n",
            "  - name: pure\n",
            "    command: echo pure\n",
        ),
        version
    )
}

/// A command load must record exactly one decision under its own outcome.
///
/// The admitted and rejected cases differ only in the declaration and in what
/// the load is expected to do, so they are one parameterized case. Splitting
/// them duplicated the recorder plumbing and let the two assertions drift
/// apart without either failing.
#[rstest]
#[case::admitted("1.0.0", true, "admitted")]
#[case::rejected("1.1.0", false, "unsupported")]
fn command_load_records_one_decision(
    #[case] version: &str,
    #[case] should_load: bool,
    #[case] expected_outcome: &str,
) -> AnyResult<()> {
    let yaml = command_load_manifest(version);
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let outcome = metrics::with_local_recorder(&recorder, || {
        from_str_with_limits(&yaml, ManifestBudgetLimits::default()).map(drop)
    });
    // A rejection is expected for the unsupported case, but the load's result
    // is not what this test measures: the counter it left behind is. Checking
    // it outside the recorder closure keeps the assertion reportable as an
    // error rather than a panic inside a `Result`-returning function.
    ensure!(
        outcome.is_ok() == should_load,
        "the {version} manifest must {} load: {outcome:?}",
        if should_load { "" } else { "not" }
    );
    let counts = admission_counts(&snapshotter.snapshot().into_vec());
    ensure!(
        counts == [(expected_outcome.to_owned(), 1)],
        "a command load must record one {expected_outcome} decision: {counts:?}"
    );
    Ok(())
}

/// A `netsuke test` load must not move the admission counter.
///
/// The test surface shares the loader with command loads, so it reaches the
/// same gate; only the counting is withheld. A test run is not an admission of
/// a user's manifest.
#[test]
fn test_surface_load_does_not_record_schema_admission() -> AnyResult<()> {
    let yaml = command_load_manifest("1.0.0");
    let surface = TestSurface::new(StdlibConfig::from_current_dir()?);
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || {
        from_str_with_options(
            &yaml,
            &ManifestName::new("Netsukefile"),
            ManifestLoadOptions::test(surface),
        )?;
        Ok::<_, anyhow::Error>(())
    })?;
    let counts = admission_counts(&snapshotter.snapshot().into_vec());
    ensure!(
        counts.is_empty(),
        "a test-surface load must not record a schema admission: {counts:?}"
    );
    Ok(())
}

/// A discovery query must not emit the admission span or decision event.
///
/// The counter is only half of what recording does; `record_admission` also
/// opens `manifest.schema.admission` and emits a decision event. The span and
/// the event are separate surfaces, so each is asserted: a regression that
/// emitted the event without opening the span would pass a span-only test, and
/// that event alone is enough to make the query path observable.
#[test]
fn manifest_query_does_not_emit_admission_traces() -> AnyResult<()> {
    let (_temp, path) = workspace_with("1.0.0")?;
    let (spans, events) = with_test_subscriber(LevelFilter::TRACE, |captured| {
        from_path_for_manifest_query(&path, None)?;
        Ok::<_, anyhow::Error>((captured.span_fields(ADMISSION_SPAN), captured.snapshot()))
    })?;
    ensure!(
        spans.is_empty(),
        "a manifest query must not open the admission span: {spans:?}"
    );
    let decisions: Vec<_> = events
        .iter()
        .filter(|event| event.contains(ADMISSION_EVENT))
        .collect();
    ensure!(
        decisions.is_empty(),
        "a manifest query must not emit an admission decision event: {decisions:?}"
    );
    Ok(())
}

/// A command load must emit both admission surfaces with their bounded fields.
///
/// This is the positive control for
/// [`manifest_query_does_not_emit_admission_traces`]: without it, an absent
/// span *and* an absent event would be equally explained by a subscriber that
/// never captured anything. It therefore asserts each surface the query test
/// requires to be missing, so neither absence can pass vacuously.
#[test]
fn command_load_emits_the_admission_span_and_event() -> AnyResult<()> {
    let yaml = command_load_manifest("1.0.0");
    let (spans, events) = with_test_subscriber(LevelFilter::TRACE, |captured| {
        from_str_with_limits(&yaml, ManifestBudgetLimits::default())?;
        Ok::<_, anyhow::Error>((captured.span_fields(ADMISSION_SPAN), captured.snapshot()))
    })?;
    ensure!(
        !spans.is_empty(),
        "a command load must open the admission span"
    );
    let rendered = spans.join(" ");
    ensure!(
        rendered.contains("outcome=\"admitted\""),
        "the admission span must carry the bounded outcome: {rendered}"
    );
    ensure!(
        rendered.contains("operation=\"admit_schema\""),
        "the admission span must name the operation: {rendered}"
    );
    let decisions: Vec<_> = events
        .iter()
        .filter(|event| event.contains(ADMISSION_EVENT))
        .collect();
    ensure!(
        !decisions.is_empty(),
        "a command load must emit the admission decision event: {events:?}"
    );
    ensure!(
        decisions
            .iter()
            .all(|event| event.contains("outcome=\"admitted\"")),
        "the decision event must carry the bounded outcome: {decisions:?}"
    );
    Ok(())
}
