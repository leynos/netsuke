//! Regression coverage for bounded configuration observability.

use super::*;
use metrics::{SharedString, Unit, gauge};
use metrics_util::{
    CompositeKey, MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use rstest::rstest;

type SnapshotEntry = (CompositeKey, Option<Unit>, Option<SharedString>, DebugValue);

/// Exact bounded label pair expected on a configuration-load counter.
///
/// This test-local value keeps assertion helpers free of unstructured
/// string arguments. Production metric composition remains in
/// [`record_config_load`].
#[derive(Clone, Copy)]
struct CounterLabels {
    phase: &'static str,
    outcome: &'static str,
}

/// One exact metric-label key-value pair expected in a snapshot entry.
#[derive(Clone, Copy)]
struct LabelExpectation {
    key: &'static str,
    value: &'static str,
}

const DIAG_MODE_SUCCESS: CounterLabels = CounterLabels {
    phase: DIAG_MODE_PHASE,
    outcome: "success",
};
const MERGE_FAILURE: CounterLabels = CounterLabels {
    phase: MERGE_PHASE,
    outcome: "failure",
};
const MERGE_SUCCESS: CounterLabels = CounterLabels {
    phase: MERGE_PHASE,
    outcome: "success",
};
const DIAG_MODE_LABEL: LabelExpectation = LabelExpectation {
    key: "phase",
    value: DIAG_MODE_PHASE,
};
const MERGE_LABEL: LabelExpectation = LabelExpectation {
    key: "phase",
    value: MERGE_PHASE,
};

fn assert_exact_config_metric_series(snapshot: &[SnapshotEntry]) {
    assert_eq!(
        count_config_load_counter_records(snapshot),
        2,
        "expected exactly two configuration-load counter series",
    );
    assert_eq!(
        count_config_load_duration_records(snapshot),
        2,
        "expected exactly two configuration-load duration series",
    );
}

fn assert_one_counter_record(snapshot: &[SnapshotEntry], labels: CounterLabels) {
    assert_eq!(
        snapshot
            .iter()
            .filter(|entry| is_counter_record(entry, labels))
            .count(),
        1,
        "expected one configuration-load counter record for phase {} and outcome {}",
        labels.phase,
        labels.outcome,
    );
}

fn is_counter_record(entry: &SnapshotEntry, labels: CounterLabels) -> bool {
    let expected_labels = [
        LabelExpectation {
            key: "phase",
            value: labels.phase,
        },
        LabelExpectation {
            key: "outcome",
            value: labels.outcome,
        },
    ];
    entry.0.kind() == MetricKind::Counter
        && entry.0.key().name() == CONFIG_LOAD_COUNTER
        && has_exact_labels(entry, &expected_labels)
        && matches!(entry.3, DebugValue::Counter(1))
}

fn has_exact_labels(entry: &SnapshotEntry, expected: &[LabelExpectation]) -> bool {
    entry.0.key().labels().count() == expected.len()
        && expected.iter().all(|label| has_label(entry, *label))
}

fn has_label(entry: &SnapshotEntry, expected: LabelExpectation) -> bool {
    entry
        .0
        .key()
        .labels()
        .any(|label| label.key() == expected.key && label.value() == expected.value)
}

fn assert_one_single_sample_duration_record(
    snapshot: &[SnapshotEntry],
    expected_phase: LabelExpectation,
    expected_seconds: f64,
) {
    assert_eq!(
        snapshot
            .iter()
            .filter(|entry| {
                is_single_sample_duration_record(entry, expected_phase, expected_seconds)
            })
            .count(),
        1,
        "expected one configuration-load duration record for phase {}",
        expected_phase.value,
    );
}

fn count_single_sample_duration_records(snapshot: &[SnapshotEntry]) -> usize {
    snapshot
        .iter()
        .filter(|entry| {
            entry.0.kind() == MetricKind::Histogram
                && entry.0.key().name() == CONFIG_LOAD_DURATION
                && matches!(entry.3, DebugValue::Histogram(ref values) if values.len() == 1)
        })
        .count()
}

fn count_config_load_counter_records(snapshot: &[SnapshotEntry]) -> usize {
    snapshot
        .iter()
        .filter(|entry| entry.0.key().name() == CONFIG_LOAD_COUNTER)
        .count()
}

fn count_config_load_duration_records(snapshot: &[SnapshotEntry]) -> usize {
    snapshot
        .iter()
        .filter(|entry| entry.0.key().name() == CONFIG_LOAD_DURATION)
        .count()
}

fn is_single_sample_duration_record(
    entry: &SnapshotEntry,
    expected_phase: LabelExpectation,
    expected_seconds: f64,
) -> bool {
    entry.0.kind() == MetricKind::Histogram
        && entry.0.key().name() == CONFIG_LOAD_DURATION
        && has_exact_labels(entry, &[expected_phase])
        && matches!(entry.3, DebugValue::Histogram(ref values) if values.as_slice() == [expected_seconds])
}

#[test]
fn configuration_metrics_recorder_discards_unrelated_metrics() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        counter!(CONFIG_LOAD_COUNTER, "phase" => DIAG_MODE_PHASE, "outcome" => "success")
            .increment(1);
        histogram!(CONFIG_LOAD_DURATION, "phase" => DIAG_MODE_PHASE).record(0.01);
        counter!("help_targets_total", "topic" => "help").increment(1);
        for _ in 0..1_000 {
            histogram!("template_render_duration_seconds", "template" => "unbounded").record(0.5);
        }
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        2,
        "only configuration metrics should be retained"
    );
    assert_one_counter_record(&snapshot, DIAG_MODE_SUCCESS);
    assert_one_single_sample_duration_record(&snapshot, DIAG_MODE_LABEL, 0.01);
    assert!(
        snapshot.iter().all(|entry| {
            matches!(
                entry.0.key().name(),
                CONFIG_LOAD_COUNTER | CONFIG_LOAD_DURATION
            )
        }),
        "snapshot must retain only configuration metric series",
    );
    assert!(
        !snapshot
            .iter()
            .any(|entry| entry.0.key().name() == "help_targets_total"),
        "rejected counter must not appear in the snapshot",
    );
    assert!(
        !snapshot
            .iter()
            .any(|entry| entry.0.key().name() == "template_render_duration_seconds"),
        "rejected histogram samples must not be retained",
    );
}

/// Same-name series with a wrong kind, extra or missing labels, or
/// unbounded label values are rejected while the valid series remain.
#[test]
fn recorder_rejects_same_name_series_with_invalid_kinds_and_labels() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        // Admitted exact series.
        counter!(
            CONFIG_LOAD_COUNTER,
            "phase" => DIAG_MODE_PHASE,
            "outcome" => "success"
        )
        .increment(1);
        counter!(CONFIG_LOAD_COUNTER, "phase" => MERGE_PHASE, "outcome" => "success").increment(1);
        histogram!(CONFIG_LOAD_DURATION, "phase" => DIAG_MODE_PHASE).record(0.01);
        histogram!(CONFIG_LOAD_DURATION, "phase" => MERGE_PHASE).record(0.02);

        // Same-name series with a wrong kind.
        gauge!(CONFIG_LOAD_COUNTER).set(1.0);
        counter!(CONFIG_LOAD_DURATION).increment(1);
        histogram!(CONFIG_LOAD_COUNTER, "phase" => MERGE_PHASE).record(0.03);

        // Same-name series with extra, missing, or unbounded labels.
        counter!(
            CONFIG_LOAD_COUNTER,
            "phase" => DIAG_MODE_PHASE,
            "outcome" => "success",
            "template" => "unbounded"
        )
        .increment(1);
        counter!(CONFIG_LOAD_COUNTER, "phase" => DIAG_MODE_PHASE).increment(1);
        counter!(CONFIG_LOAD_COUNTER, "phase" => "user_phase", "outcome" => "success").increment(1);
        histogram!(CONFIG_LOAD_DURATION, "phase" => DIAG_MODE_PHASE, "extra" => "label")
            .record(0.04);
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_exact_config_metric_series(&snapshot);
    assert_one_counter_record(&snapshot, DIAG_MODE_SUCCESS);
    assert_one_counter_record(&snapshot, MERGE_SUCCESS);
    assert_one_single_sample_duration_record(&snapshot, DIAG_MODE_LABEL, 0.01);
    assert_one_single_sample_duration_record(&snapshot, MERGE_LABEL, 0.02);
    assert!(
        !snapshot
            .iter()
            .any(|entry| entry.0.kind() == MetricKind::Gauge),
        "a gauge with a configuration name must be rejected",
    );
}

#[rstest]
fn classifies_config_errors_without_exposing_details() {
    let file_error = OrthoError::File {
        path: "private.toml".into(),
        source: Box::new(std::io::Error::other("read failure")),
    };
    let validation_error = OrthoError::Validation {
        key: "jobs".into(),
        message: "must be positive".into(),
    };
    let fallback_error = std::io::Error::other("unknown source");

    assert_eq!(classify_error(&file_error), "io");
    assert_eq!(classify_error(&validation_error), "validation");
    assert_eq!(classify_error(&fallback_error), "parse");
}

#[rstest]
fn records_each_config_load_phase_and_outcome() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let diag_mode_clock = monotony::test_util::FixedMonotonicClock::with_elapsed(
        std::time::Duration::from_millis(10),
    );
    let merge_clock = monotony::test_util::FixedMonotonicClock::with_elapsed(
        std::time::Duration::from_millis(20),
    );

    metrics::with_local_recorder(&recorder, || {
        let success = record_config_load(ConfigLoadPhase::DiagMode, &diag_mode_clock, || {
            Ok::<_, ()>(())
        });
        let failure = record_config_load(ConfigLoadPhase::Merge, &merge_clock, || Err::<(), _>(()));

        assert!(success.is_ok());
        assert!(failure.is_err());
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_exact_config_metric_series(&snapshot);
    assert_one_counter_record(&snapshot, DIAG_MODE_SUCCESS);
    assert_one_counter_record(&snapshot, MERGE_FAILURE);
    assert_one_single_sample_duration_record(&snapshot, DIAG_MODE_LABEL, 0.01);
    assert_one_single_sample_duration_record(&snapshot, MERGE_LABEL, 0.02);
    assert_eq!(count_single_sample_duration_records(&snapshot), 2);
}

/// A snapshot drains the recorder: a later snapshot keeps the bounded series
/// but resets every recorded sample.
#[test]
fn emitting_a_snapshot_drains_recorded_samples() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        counter!(
            CONFIG_LOAD_COUNTER,
            "phase" => DIAG_MODE_PHASE,
            "outcome" => "success"
        )
        .increment(1);
        histogram!(CONFIG_LOAD_DURATION, "phase" => DIAG_MODE_PHASE).record(0.01);
    });

    let first = snapshotter.snapshot().into_vec();
    assert_one_counter_record(&first, DIAG_MODE_SUCCESS);
    assert_one_single_sample_duration_record(&first, DIAG_MODE_LABEL, 0.01);

    let second = snapshotter.snapshot().into_vec();
    assert!(
        !second
            .iter()
            .any(|entry| is_counter_record(entry, DIAG_MODE_SUCCESS)),
        "the drained snapshot must not retain the counter sample: {second:?}",
    );
    assert!(
        !second
            .iter()
            .any(|entry| is_single_sample_duration_record(entry, DIAG_MODE_LABEL, 0.01)),
        "the drained snapshot must not retain the histogram sample: {second:?}",
    );
    assert_eq!(
        count_config_load_counter_records(&second),
        1,
        "the drained snapshot keeps the counter series",
    );
    assert_eq!(
        count_config_load_duration_records(&second),
        1,
        "the drained snapshot keeps the duration series",
    );
}

/// Concurrent threads incrementing a shared configuration counter accumulate
/// every increment without interference.
#[test]
fn concurrent_increments_accumulate_on_the_shared_recorder() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();
    let counter = std::sync::Arc::new(metrics::with_local_recorder(&recorder, || {
        metrics::counter!(
            CONFIG_LOAD_COUNTER,
            "phase" => DIAG_MODE_PHASE,
            "outcome" => "success"
        )
    }));

    let threads: Vec<_> = (0..8)
        .map(|_| {
            let handle = std::sync::Arc::clone(&counter);
            std::thread::spawn(move || handle.increment(1))
        })
        .collect();
    for thread in threads {
        thread.join().expect("increment thread");
    }

    let snapshot = snapshotter.snapshot().into_vec();
    let accumulated = snapshot
        .iter()
        .find(|entry| entry.0.key().name() == CONFIG_LOAD_COUNTER);
    let Some(sample) = accumulated else {
        panic!("expected an accumulated configuration counter: {snapshot:?}");
    };
    assert!(
        matches!(sample.3, DebugValue::Counter(8)),
        "all eight increments should reach the shared counter: {sample:?}",
    );
}
