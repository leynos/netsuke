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
const REJECTIONS_TOTAL: &str = "netsuke_manifest_glob_rejections_total";

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

/// Value of the glob-rejection counter carrying its bounded labels.
fn unsafe_path_rejection_count(snapshot: &Snapshot) -> Option<u64> {
    snapshot.iter().find_map(|(key, _, _, value)| {
        if key.kind() != MetricKind::Counter || key.key().name() != REJECTIONS_TOTAL {
            return None;
        }
        let has_outcome = key
            .key()
            .labels()
            .any(|label| label.key() == "outcome" && label.value() == "unsafe_path");
        let has_error_category = key.key().labels().any(|label| {
            label.key() == "error_category" && label.value() == "shell_quoting_required"
        });
        match value {
            DebugValue::Counter(count) if has_outcome && has_error_category => Some(*count),
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

#[cfg(unix)]
#[rstest]
fn jinja_glob_adapter_records_a_redacted_unsafe_path_rejection() -> Result<()> {
    let temp = tempdir()?;
    let unsafe_name = "secret; touch PWNED; #.txt";
    test_support::fs::write(temp.path().join(unsafe_name), "unsafe path")?;
    let pattern = format!("{}/*.txt", temp.path().display());
    let yaml = manifest_yaml(&format!(
        "targets:\n  - foreach: glob({pattern:?})\n    name: unsafe\n    command: echo {{{{ item }}}}\n"
    ));

    let (manifest, events, snapshot) = recorded(|| from_str(&yaml));
    let error = manifest.expect_err("the unsafe path must stop manifest loading");
    ensure!(
        format!("{error:#}").contains("characters that require shell quoting"),
        "the rejection should retain its semantic diagnostic: {error:#}"
    );
    ensure!(
        unsafe_path_rejection_count(&snapshot) == Some(1),
        "the adapter must count the categorized rejection: {snapshot:?}"
    );
    let event = events
        .iter()
        .find(|event| event.contains("glob template path rejected"))
        .context("the Jinja adapter must emit a rejection event")?;
    ensure!(
        event.contains("path=\"<redacted>\"")
            && event.contains("outcome=\"unsafe_path\"")
            && event.contains("error_category=\"shell_quoting_required\""),
        "the rejection event must retain bounded fields: {event}"
    );
    ensure!(
        !event.contains(unsafe_name) && !event.contains(&temp.path().display().to_string()),
        "the rejection event must not disclose the matched path: {event}"
    );
    Ok(())
}
