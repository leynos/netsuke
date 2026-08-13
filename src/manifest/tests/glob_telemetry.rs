//! Telemetry coverage for the manifest Jinja `glob()` adapter.
//!
//! This test reaches `from_str` rather than glob's private query and recorder,
//! pinning that the adapter records the bounded observations it receives.

use super::super::from_str;
use crate::test_tracing_capture::with_test_subscriber;
use anyhow::{Context, Result, ensure};
use metrics::SharedString;
use metrics_util::{
    CompositeKey, MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use rstest::rstest;
use tempfile::tempdir;
use test_support::manifest::manifest_yaml;
use tracing::level_filters::LevelFilter;

const EXPANSIONS_TOTAL: &str = "netsuke_manifest_glob_expansions_total";

type Snapshot = Vec<(
    CompositeKey,
    Option<metrics::Unit>,
    Option<SharedString>,
    DebugValue,
)>;

/// Run `parse` with local metrics and tracing capture.
fn recorded<T>(parse: impl FnOnce() -> T) -> (T, Vec<String>, Snapshot) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let (value, events) = metrics::with_local_recorder(&recorder, || {
        with_test_subscriber(LevelFilter::DEBUG, |captured| {
            let value = parse();
            (value, captured.snapshot())
        })
    });
    (value, events, snapshotter.snapshot().into_vec())
}

/// Value of the glob-expansion counter labelled with `outcome`.
fn expansion_count(snapshot: &Snapshot, outcome: &str) -> Option<u64> {
    snapshot.iter().find_map(|(key, _, _, value)| {
        if key.kind() != MetricKind::Counter || key.key().name() != EXPANSIONS_TOTAL {
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

#[rstest]
fn jinja_glob_adapter_records_an_unopenable_prefix() -> Result<()> {
    let temp = tempdir()?;
    let pattern = format!("{}/missing/*.txt", temp.path().display());
    let yaml = manifest_yaml(&format!(
        "targets:\n  - foreach: glob('{pattern}')\n    name: no-match\n    command: echo hi\n"
    ));

    let (manifest, events, snapshot) = recorded(|| from_str(&yaml));
    ensure!(
        manifest?.targets.is_empty(),
        "the missing prefix has no matches"
    );
    ensure!(
        expansion_count(&snapshot, "unopenable_prefix") == Some(1),
        "the Jinja adapter must record the unopenable prefix: {snapshot:?}"
    );
    let event = events
        .iter()
        .find(|event| event.contains("glob literal prefix names no directory"))
        .context("the Jinja adapter must emit its trace event")?;
    ensure!(
        event.contains("pattern=\"<redacted>\"") && event.contains("prefix=\"<redacted>\""),
        "the adapter event must retain bounded fields: {event}"
    );
    ensure!(
        !event.contains(&temp.path().display().to_string()),
        "the adapter event must not disclose its absolute path: {event}"
    );
    Ok(())
}
