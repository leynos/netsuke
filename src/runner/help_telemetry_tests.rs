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

/// The fixed result labels expected for one help-targets telemetry scenario.
struct ExpectedHelpTargetsTelemetry {
    outcome: &'static str,
    error_category: &'static str,
    succeeds: bool,
}

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

/// Find one metric with the exact bounded outcome and error-category labels.
fn metric_value<'snapshot>(
    snapshot: &'snapshot Snapshot,
    kind: MetricKind,
    name: &str,
    expected: &ExpectedHelpTargetsTelemetry,
) -> Option<&'snapshot DebugValue> {
    snapshot
        .iter()
        .find_map(|(key, _unit, _description, value)| {
            if key.kind() != kind || key.key().name() != name {
                return None;
            }
            let labels: Vec<(&str, &str)> = key
                .key()
                .labels()
                .map(|label| (label.key(), label.value()))
                .collect();
            let matches = labels.len() == 2
                && labels.contains(&("outcome", expected.outcome))
                && labels.contains(&("error_category", expected.error_category));
            matches.then_some(value)
        })
}

#[test]
fn metric_value_rejects_metrics_with_extra_labels() {
    let expected = ExpectedHelpTargetsTelemetry {
        outcome: "success",
        error_category: "none",
        succeeds: true,
    };
    let snapshot = vec![(
        metrics_util::CompositeKey::new(
            MetricKind::Counter,
            metrics::Key::from_parts(
                HELP_TARGETS_TOTAL,
                vec![
                    metrics::Label::new("outcome", "success"),
                    metrics::Label::new("error_category", "none"),
                    metrics::Label::new("operation", "query"),
                ],
            ),
        ),
        None,
        None,
        DebugValue::Counter(1),
    )];

    assert!(
        metric_value(
            &snapshot,
            MetricKind::Counter,
            HELP_TARGETS_TOTAL,
            &expected
        )
        .is_none()
    );
}

/// Assert the complete bounded telemetry contract for one help-targets query.
fn assert_help_targets_telemetry(
    cli: &Cli,
    expected: &ExpectedHelpTargetsTelemetry,
    scenario: &str,
) -> Result<()> {
    let _lock = localizer_test_lock().map_err(|error| anyhow::anyhow!("{error}"))?;
    let _guard = set_localizer_for_tests(Arc::from(crate::cli_localization::build_localizer(
        Some("en-US"),
    )));
    let ((result, events), snapshot) = recorded(|| {
        with_test_subscriber(LevelFilter::INFO, |captured| {
            let result = handle_help_targets(cli, &SilentReporter);
            (result, captured.snapshot())
        })
    });

    match (expected.succeeds, result) {
        (true, Ok(())) | (false, Err(_)) => {}
        (true, Err(error)) => {
            anyhow::bail!("{scenario} should succeed: {error:?}");
        }
        (false, Ok(())) => {
            anyhow::bail!("{scenario} should fail");
        }
    }

    let counter = metric_value(&snapshot, MetricKind::Counter, HELP_TARGETS_TOTAL, expected);
    ensure!(
        matches!(counter, Some(DebugValue::Counter(1))),
        "{scenario} should record one counter for outcome={:?}, error_category={:?}: {snapshot:?}",
        expected.outcome,
        expected.error_category,
    );

    let duration = metric_value(
        &snapshot,
        MetricKind::Histogram,
        HELP_TARGETS_DURATION,
        expected,
    );
    ensure!(
        matches!(duration, Some(DebugValue::Histogram(samples)) if samples.len() == 1),
        "{scenario} should record one duration sample for outcome={:?}, error_category={:?}: {snapshot:?}",
        expected.outcome,
        expected.error_category,
    );
    ensure!(
        events
            .iter()
            .any(|event| event.contains("Completed help targets query")
                && event.contains(&format!("outcome=\"{}\"", expected.outcome))
                && event.contains(&format!("error_category=\"{}\"", expected.error_category))),
        "{scenario} should emit a completion event for outcome={:?}, error_category={:?}: {events:?}; metrics: {snapshot:?}",
        expected.outcome,
        expected.error_category,
    );
    Ok(())
}

#[test]
fn help_targets_records_bounded_success_telemetry() -> Result<()> {
    let (_temp, cli) = help_targets_fixture()?;
    assert_help_targets_telemetry(
        &cli,
        &ExpectedHelpTargetsTelemetry {
            outcome: "success",
            error_category: "none",
            succeeds: true,
        },
        "successful help targets",
    )
}

#[test]
fn help_targets_records_manifest_failure_telemetry() -> Result<()> {
    let cli = Cli {
        file: "missing-help-telemetry-manifest.yml".into(),
        ..Cli::default()
    };
    assert_help_targets_telemetry(
        &cli,
        &ExpectedHelpTargetsTelemetry {
            outcome: "error",
            error_category: "manifest_not_found",
            succeeds: false,
        },
        "missing manifest help targets",
    )
}

#[test]
fn help_targets_records_other_failure_telemetry() -> Result<()> {
    let (_temp, cli) = help_targets_fixture_with_manifest(INVALID_MANIFEST)?;
    assert_help_targets_telemetry(
        &cli,
        &ExpectedHelpTargetsTelemetry {
            outcome: "error",
            error_category: "other",
            succeeds: false,
        },
        "invalid manifest help targets",
    )
}
