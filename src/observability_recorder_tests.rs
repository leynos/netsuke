//! Process-recorder coverage for bounded application metrics.
//!
//! These tests exercise the application-owned [`ConfigMetricsRecorder`] that
//! `main` installs globally. They verify its snapshot retains the bounded
//! configuration and timing-summary series while rejecting unrelated workload
//! observations.

use super::*;
use metrics::{SharedString, Unit};
use metrics_util::{CompositeKey, MetricKind, debugging::DebugValue};
use netsuke::{
    cli::PATH_VALIDATION_TOTAL,
    runner::{BASH_PREFLIGHT_TOTAL, RECIPE_SHELL_RESOLUTIONS_TOTAL},
};

type SnapshotEntry = (CompositeKey, Option<Unit>, Option<SharedString>, DebugValue);
/// Fixed three-label layout used by recipe-shell counters.
type MetricLabels = [(&'static str, &'static str); 3];

/// Cover bounded legacy-recipe operation metric registrations separately.
#[path = "observability_recorder_legacy_recipe_tests.rs"]
mod legacy_recipe_tests;

/// Define the rejected label variants for recipe-shell resolution metrics.
const INVALID_RECIPE_SHELL_RESOLUTION_SERIES: [MetricLabels; 3] = [
    [
        ("recipe_shell", "unbounded"),
        ("outcome", "success"),
        ("error_category", "none"),
    ],
    [
        ("recipe_shell", "powershell"),
        ("outcome", "unbounded"),
        ("error_category", "none"),
    ],
    [
        ("recipe_shell", "powershell"),
        ("outcome", "success"),
        ("error_category", "unbounded"),
    ],
];

/// Define the rejected label variants for Bash preflight metrics.
const INVALID_BASH_PREFLIGHT_SERIES: [MetricLabels; 3] = [
    [
        ("recipe_shell", "powershell"),
        ("outcome", "error"),
        ("probe_outcome", "not_found"),
    ],
    [
        ("recipe_shell", "bash"),
        ("outcome", "unbounded"),
        ("probe_outcome", "not_found"),
    ],
    [
        ("recipe_shell", "bash"),
        ("outcome", "error"),
        ("probe_outcome", "unbounded"),
    ],
];

/// Record the valid, invalid, and unrelated series used to verify filtering.
fn record_mixed_metric_series(recorder: &ConfigMetricsRecorder) {
    metrics::with_local_recorder(recorder, || {
        counter!(CONFIG_LOAD_COUNTER, "phase" => DIAG_MODE_PHASE, "outcome" => "success")
            .increment(1);
        histogram!(CONFIG_LOAD_DURATION, "phase" => DIAG_MODE_PHASE).record(0.01);
        counter!(STARTUP_CONFIG_LOAD_COUNTER, "outcome" => "failure").increment(1);
        histogram!(STARTUP_CONFIG_LOAD_DURATION).record(0.02);
        counter!(STARTUP_CONFIG_LOAD_COUNTER).increment(1);
        histogram!(STARTUP_CONFIG_LOAD_DURATION, "phase" => DIAG_MODE_PHASE).record(0.03);
        counter!("help_targets_total", "topic" => "help").increment(1);
        histogram!("template_render_duration_seconds", "template" => "unbounded").record(0.5);
    });
}

/// Assert the retained startup counter has its sole bounded failure label.
fn assert_retained_startup_counter(snapshot: &[SnapshotEntry]) {
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Counter
                && entry.0.key().name() == STARTUP_CONFIG_LOAD_COUNTER
                && matches!(
                    entry.0.key().labels().collect::<Vec<_>>().as_slice(),
                    [label] if label.key() == "outcome" && label.value() == "failure"
                )
                && matches!(entry.3, DebugValue::Counter(1))
        }),
        "snapshot should retain the bounded startup counter: {snapshot:?}"
    );
}

/// Assert an unlabelled duration has the exact expected histogram sample.
fn assert_unlabelled_duration(
    snapshot: &[SnapshotEntry],
    metric_name: &str,
    expected_seconds: f64,
) {
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Histogram
                && entry.0.key().name() == metric_name
                && entry.0.key().labels().next().is_none()
                && matches!(entry.3, DebugValue::Histogram(ref values) if values.as_slice() == [expected_seconds])
        }),
        "expected unlabelled duration {metric_name} with sample [{expected_seconds}]: {snapshot:?}"
    );
}

/// Assert every retained series belongs to the bounded configuration vocabulary.
fn assert_only_configuration_metric_names(snapshot: &[SnapshotEntry]) {
    assert!(
        snapshot.iter().all(|entry| {
            matches!(
                entry.0.key().name(),
                CONFIG_LOAD_COUNTER
                    | CONFIG_LOAD_DURATION
                    | STARTUP_CONFIG_LOAD_COUNTER
                    | STARTUP_CONFIG_LOAD_DURATION
            )
        }),
        "snapshot must reject unrelated metric series: {snapshot:?}"
    );
}

#[test]
fn configuration_metrics_recorder_retains_bounded_startup_and_phase_series() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    record_mixed_metric_series(&recorder);

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        4,
        "only bounded configuration metric series should be retained"
    );
    assert_retained_startup_counter(&snapshot);
    assert_unlabelled_duration(&snapshot, STARTUP_CONFIG_LOAD_DURATION, 0.02);
    assert_only_configuration_metric_names(&snapshot);
}

/// The application recorder retains the bounded timing-summary sink series.
#[test]
fn recorder_retains_bounded_timing_summary_sink_series() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        counter!(TIMING_SUMMARY_SINK_WRITES_TOTAL, "outcome" => "success").increment(1);
        counter!(TIMING_SUMMARY_SINK_WRITES_TOTAL, "outcome" => "write_error").increment(1);
        histogram!(TIMING_SUMMARY_SINK_WRITE_DURATION).record(0.01);
        counter!(TIMING_SUMMARY_SINK_WRITES_TOTAL, "outcome" => "unbounded").increment(1);
        histogram!(TIMING_SUMMARY_SINK_WRITE_DURATION, "outcome" => "success").record(0.02);
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        3,
        "only the two bounded timing outcomes and unlabelled duration are retained"
    );
    assert_timing_summary_counter(&snapshot, "success");
    assert_timing_summary_counter(&snapshot, "write_error");
    assert_unlabelled_duration(&snapshot, TIMING_SUMMARY_SINK_WRITE_DURATION, 0.01);
}

/// Retain only unlabelled manifest-filtering counter series.
#[test]
fn recorder_retains_unlabelled_manifest_filtering_counters() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();
    let expected = [
        ("netsuke_manifest_filtered_targets_total", 65),
        ("netsuke_manifest_filtered_actions_total", 2),
        ("netsuke_manifest_omitted_filtered_entries_total", 3),
    ];

    metrics::with_local_recorder(&recorder, || {
        for (name, value) in expected {
            counter!(name).increment(value);
            counter!(name, "section" => "targets").increment(value);
        }
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        expected.len(),
        "only unlabelled manifest-filtering counter series should be retained"
    );
    for (name, value) in expected {
        assert!(
            snapshot.iter().any(|entry| {
                entry.0.kind() == MetricKind::Counter
                    && entry.0.key().name() == name
                    && entry.0.key().labels().next().is_none()
                    && matches!(entry.3, DebugValue::Counter(observed) if observed == value)
            }),
            "expected unlabelled manifest-filtering counter {name}={value}: {snapshot:?}"
        );
    }
}

/// Retain only the fixed source and reason labels for CLI path-validation counters.
#[test]
fn recorder_retains_bounded_cli_path_validation_series() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        counter!(PATH_VALIDATION_TOTAL, "source" => "file", "reason" => "non_utf8").increment(1);
        counter!(PATH_VALIDATION_TOTAL, "source" => "directory", "reason" => "non_utf8")
            .increment(1);
        counter!(PATH_VALIDATION_TOTAL, "source" => "unbounded", "reason" => "non_utf8")
            .increment(1);
        counter!(PATH_VALIDATION_TOTAL, "source" => "file", "reason" => "unbounded").increment(1);
        counter!(PATH_VALIDATION_TOTAL, "source" => "file").increment(1);
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        2,
        "only the two bounded CLI path-validation series are retained"
    );
    for source in ["file", "directory"] {
        assert_path_validation_counter(&snapshot, source);
    }
}

/// Verify one retained CLI path-validation counter with the specified source and the `non_utf8` reason.
fn assert_path_validation_counter(snapshot: &[SnapshotEntry], source: &str) {
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Counter
                && entry.0.key().name() == PATH_VALIDATION_TOTAL
                && entry.0.key().labels().count() == 2
                && entry
                    .0
                    .key()
                    .labels()
                    .any(|label| label.key() == "source" && label.value() == source)
                && entry
                    .0
                    .key()
                    .labels()
                    .any(|label| label.key() == "reason" && label.value() == "non_utf8")
                && matches!(entry.3, DebugValue::Counter(1))
        }),
        "recorder should retain the {source:?} path-validation series: {snapshot:?}"
    );
}
/// Record the two valid counter series emitted for recipe-shell operations.
fn record_valid_recipe_shell_series() {
    counter!(
        RECIPE_SHELL_RESOLUTIONS_TOTAL,
        "recipe_shell" => "powershell",
        "outcome" => "success",
        "error_category" => "none"
    )
    .increment(1);
    counter!(
        BASH_PREFLIGHT_TOTAL,
        "recipe_shell" => "bash",
        "outcome" => "error",
        "probe_outcome" => "not_found"
    )
    .increment(1);
}

/// Record every invalid fixed-shape series for one recipe-shell counter.
fn record_invalid_recipe_shell_series(metric_name: &'static str, series: &[MetricLabels]) {
    for &[
        (first_key, first_value),
        (second_key, second_value),
        (third_key, third_value),
    ] in series
    {
        counter!(
            metric_name,
            first_key => first_value,
            second_key => second_value,
            third_key => third_value
        )
        .increment(1);
    }
}

/// Retain only the fixed recipe-shell counter vocabulary exposed by the runner.
#[test]
fn recorder_retains_bounded_recipe_shell_series() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        record_valid_recipe_shell_series();
        record_invalid_recipe_shell_series(
            RECIPE_SHELL_RESOLUTIONS_TOTAL,
            &INVALID_RECIPE_SHELL_RESOLUTION_SERIES,
        );
        record_invalid_recipe_shell_series(BASH_PREFLIGHT_TOTAL, &INVALID_BASH_PREFLIGHT_SERIES);
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        2,
        "only bounded recipe-shell series are retained"
    );
    assert_recipe_shell_counter(
        &snapshot,
        RECIPE_SHELL_RESOLUTIONS_TOTAL,
        &[
            ("recipe_shell", "powershell"),
            ("outcome", "success"),
            ("error_category", "none"),
        ],
    );
    assert_recipe_shell_counter(
        &snapshot,
        BASH_PREFLIGHT_TOTAL,
        &[
            ("recipe_shell", "bash"),
            ("outcome", "error"),
            ("probe_outcome", "not_found"),
        ],
    );
}

/// Assert that one retained recipe-shell counter has exactly `labels`.
fn assert_recipe_shell_counter(
    snapshot: &[SnapshotEntry],
    metric_name: &str,
    labels: &[(&str, &str)],
) {
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Counter
                && entry.0.key().name() == metric_name
                && entry.0.key().labels().count() == labels.len()
                && labels.iter().all(|(key, value)| {
                    entry
                        .0
                        .key()
                        .labels()
                        .any(|label| label.key() == *key && label.value() == *value)
                })
                && matches!(entry.3, DebugValue::Counter(1))
        }),
        "expected bounded {metric_name} labels {labels:?}: {snapshot:?}"
    );
}

/// Assert that `outcome` retains one bounded timing-summary counter increment.
fn assert_timing_summary_counter(snapshot: &[SnapshotEntry], outcome: &str) {
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Counter
                && entry.0.key().name() == TIMING_SUMMARY_SINK_WRITES_TOTAL
                && entry.0.key().labels().count() == 1
                && entry
                    .0
                    .key()
                    .labels()
                    .any(|label| label.key() == "outcome" && label.value() == outcome)
                && matches!(entry.3, DebugValue::Counter(1))
        }),
        "expected retained timing-summary counter with outcome {outcome}: {snapshot:?}"
    );
}
