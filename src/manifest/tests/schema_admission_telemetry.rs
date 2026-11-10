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
    test_fs::write(
        &manifest_path,
        format!(
            concat!(
                "netsuke_version: \"{}\"\n",
                "targets:\n",
                "  - name: pure\n",
                "    command: echo pure\n",
            ),
            version
        ),
    )?;
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

/// A command load must record exactly one admission decision.
#[test]
fn command_load_records_one_admitted_decision() -> AnyResult<()> {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: pure\n",
        "    command: echo pure\n",
    );
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || {
        from_str_with_limits(yaml, ManifestBudgetLimits::default())?;
        Ok::<_, anyhow::Error>(())
    })?;
    let counts = admission_counts(&snapshotter.snapshot().into_vec());
    ensure!(
        counts == [("admitted".to_owned(), 1)],
        "a command load must record one admitted decision: {counts:?}"
    );
    Ok(())
}

/// A command load must record a rejection under its own bounded outcome.
#[test]
fn command_load_records_one_rejected_decision() -> AnyResult<()> {
    let yaml = concat!(
        "netsuke_version: \"1.1.0\"\n",
        "targets:\n",
        "  - name: pure\n",
        "    command: echo pure\n",
    );
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || {
        // The load fails; the point of the test is the counter it left behind.
        from_str_with_limits(yaml, ManifestBudgetLimits::default())
            .expect_err("an unsupported schema must be rejected");
    });
    let counts = admission_counts(&snapshotter.snapshot().into_vec());
    ensure!(
        counts == [("unsupported".to_owned(), 1)],
        "a rejected command load must record one unsupported decision: {counts:?}"
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
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: pure\n",
        "    command: echo pure\n",
    );
    let surface = TestSurface::new(StdlibConfig::from_current_dir()?);
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || {
        from_str_with_options(
            yaml,
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
/// opens `manifest.schema.admission` and emits a decision event. Asserting the
/// counter alone would let a regression re-introduce the event on the query
/// path and still pass, which is precisely the side effect the registration
/// gate exists to prevent.
#[test]
fn manifest_query_does_not_emit_admission_traces() -> AnyResult<()> {
    let (_temp, path) = workspace_with("1.0.0")?;
    let spans = with_test_subscriber(LevelFilter::TRACE, |captured| {
        from_path_for_manifest_query(&path, None)?;
        Ok::<_, anyhow::Error>(captured.span_fields(ADMISSION_SPAN))
    })?;
    ensure!(
        spans.is_empty(),
        "a manifest query must not open the admission span: {spans:?}"
    );
    Ok(())
}

/// A command load must emit the admission span with its bounded outcome.
///
/// This is the positive control for
/// [`manifest_query_does_not_emit_admission_traces`]: without it, an absent
/// span would be equally explained by a subscriber that never captured
/// anything.
#[test]
fn command_load_emits_the_admission_span() -> AnyResult<()> {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: pure\n",
        "    command: echo pure\n",
    );
    let spans = with_test_subscriber(LevelFilter::TRACE, |captured| {
        from_str_with_limits(yaml, ManifestBudgetLimits::default())?;
        Ok::<_, anyhow::Error>(captured.span_fields(ADMISSION_SPAN))
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
    Ok(())
}
