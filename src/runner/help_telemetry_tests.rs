//! Telemetry coverage for the `netsuke help targets` orchestration boundary.

use super::telemetry::{HELP_TARGETS_DURATION, HELP_TARGETS_TOTAL};
use super::*;
use crate::cli::Cli;
use crate::localization::set_localizer_for_tests;
use crate::status::SilentReporter;
use crate::test_tracing_capture::with_test_subscriber;
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use metrics_util::MetricKind;
use metrics_util::debugging::{DebugValue, DebuggingRecorder};
use std::sync::Arc;
use tempfile::TempDir;
use test_support::localizer_test_lock;
use tracing_subscriber::filter::LevelFilter;

const MANIFEST: &str = r#"netsuke_version: "1.0.0"
actions:
  - name: inspect
    command: "true"
targets: []
"#;

const INVALID_MANIFEST: &str = "targets:\n\t- name: broken\n";

/// One drained metrics snapshot; the debugging snapshotter empties histogram
/// samples on read, so each test collects it exactly once.
type Snapshot = Vec<(
    metrics_util::CompositeKey,
    Option<metrics::Unit>,
    Option<metrics::SharedString>,
    DebugValue,
)>;

/// Run an operation under a local metrics recorder and return its result and
/// metrics snapshot without installing a process-wide recorder.
fn recorded<T>(operation: impl FnOnce() -> T) -> (T, Snapshot) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let result = metrics::with_local_recorder(&recorder, operation);
    (result, snapshotter.snapshot().into_vec())
}

/// Build a CLI pointing at a capability-written manifest fixture.
fn help_targets_fixture() -> Result<(TempDir, Cli)> {
    help_targets_fixture_with_manifest(MANIFEST)
}

/// Build a CLI pointing at a capability-written manifest fixture with `manifest`.
fn help_targets_fixture_with_manifest(manifest: &str) -> Result<(TempDir, Cli)> {
    let temp = TempDir::new().context("create help telemetry workspace")?;
    let root = Utf8Path::from_path(temp.path()).context("telemetry workspace path is UTF-8")?;
    let workspace = Dir::open_ambient_dir(root, ambient_authority())
        .context("open help telemetry workspace")?;
    workspace
        .write("Netsukefile", manifest)
        .context("write help telemetry manifest")?;
    let manifest_path = root.join("Netsukefile").into_std_path_buf();
    Ok((
        temp,
        Cli {
            file: manifest_path,
            ..Cli::default()
        },
    ))
}

/// Find the counter value for one bounded outcome/error category pair.
fn counter_value(snapshot: &Snapshot, outcome: &str, error_category: &str) -> Option<u64> {
    snapshot
        .iter()
        .find_map(|(key, _unit, _description, value)| {
            if key.kind() != MetricKind::Counter || key.key().name() != HELP_TARGETS_TOTAL {
                return None;
            }
            let labels: Vec<(&str, &str)> = key
                .key()
                .labels()
                .map(|label| (label.key(), label.value()))
                .collect();
            let matches = labels.contains(&("outcome", outcome))
                && labels.contains(&("error_category", error_category));
            match value {
                DebugValue::Counter(count) if matches => Some(*count),
                _ => None,
            }
        })
}

/// Count recorded duration samples for one bounded outcome/error pair.
fn duration_sample_count(snapshot: &Snapshot, outcome: &str, error_category: &str) -> usize {
    snapshot
        .iter()
        .find_map(|(key, _unit, _description, value)| {
            if key.kind() != MetricKind::Histogram || key.key().name() != HELP_TARGETS_DURATION {
                return None;
            }
            let labels: Vec<(&str, &str)> = key
                .key()
                .labels()
                .map(|label| (label.key(), label.value()))
                .collect();
            let matches = labels.contains(&("outcome", outcome))
                && labels.contains(&("error_category", error_category));
            match value {
                DebugValue::Histogram(samples) if matches => Some(samples.len()),
                _ => None,
            }
        })
        .unwrap_or_default()
}

#[test]
fn help_targets_records_bounded_success_telemetry() -> Result<()> {
    let _lock = localizer_test_lock().map_err(|error| anyhow::anyhow!("{error}"))?;
    let _guard = set_localizer_for_tests(Arc::from(crate::cli_localization::build_localizer(
        Some("en-US"),
    )));
    let (_temp, cli) = help_targets_fixture()?;
    let ((result, events), snapshot) = recorded(|| {
        with_test_subscriber(LevelFilter::INFO, |captured| {
            let result = handle_help_targets(&cli, &SilentReporter);
            (result, captured.snapshot())
        })
    });

    result?;
    ensure!(
        counter_value(&snapshot, "success", "none") == Some(1),
        "successful help targets should increment the bounded counter"
    );
    ensure!(
        duration_sample_count(&snapshot, "success", "none") == 1,
        "successful help targets should record one duration sample"
    );
    ensure!(
        events
            .iter()
            .any(|event| event.contains("Completed help targets query")
                && event.contains("outcome=\"success\"")
                && event.contains("error_category=\"none\"")),
        "successful help targets should emit a bounded completion event: {events:?}"
    );
    Ok(())
}

#[test]
fn help_targets_records_manifest_failure_telemetry() -> Result<()> {
    let _lock = localizer_test_lock().map_err(|error| anyhow::anyhow!("{error}"))?;
    let _guard = set_localizer_for_tests(Arc::from(crate::cli_localization::build_localizer(
        Some("en-US"),
    )));
    let cli = Cli {
        file: "missing-help-telemetry-manifest.yml".into(),
        ..Cli::default()
    };
    let ((result, events), snapshot) = recorded(|| {
        with_test_subscriber(LevelFilter::INFO, |captured| {
            let result = handle_help_targets(&cli, &SilentReporter);
            (result, captured.snapshot())
        })
    });

    ensure!(result.is_err(), "missing manifest should fail help targets");
    ensure!(
        counter_value(&snapshot, "error", "manifest_not_found") == Some(1),
        "missing manifest should increment the bounded failure counter"
    );
    ensure!(
        duration_sample_count(&snapshot, "error", "manifest_not_found") == 1,
        "missing manifest should record one duration sample"
    );
    ensure!(
        events
            .iter()
            .any(|event| event.contains("Completed help targets query")
                && event.contains("outcome=\"error\"")
                && event.contains("error_category=\"manifest_not_found\"")),
        "failed help targets should emit a bounded completion event: {events:?}"
    );
    Ok(())
}

#[test]
fn help_targets_records_other_failure_telemetry() -> Result<()> {
    let _lock = localizer_test_lock().map_err(|error| anyhow::anyhow!("{error}"))?;
    let _guard = set_localizer_for_tests(Arc::from(crate::cli_localization::build_localizer(
        Some("en-US"),
    )));
    let (_temp, cli) = help_targets_fixture_with_manifest(INVALID_MANIFEST)?;
    let ((result, events), snapshot) = recorded(|| {
        with_test_subscriber(LevelFilter::INFO, |captured| {
            let result = handle_help_targets(&cli, &SilentReporter);
            (result, captured.snapshot())
        })
    });

    ensure!(result.is_err(), "invalid manifest should fail help targets");
    ensure!(
        counter_value(&snapshot, "error", "other") == Some(1),
        "invalid manifest should increment the bounded other-failure counter"
    );
    ensure!(
        duration_sample_count(&snapshot, "error", "other") == 1,
        "invalid manifest should record one duration sample"
    );
    ensure!(
        events
            .iter()
            .any(|event| event.contains("Completed help targets query")
                && event.contains("outcome=\"error\"")
                && event.contains("error_category=\"other\"")),
        "invalid manifest should emit a bounded completion event: {events:?}"
    );
    Ok(())
}
