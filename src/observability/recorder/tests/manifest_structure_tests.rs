//! Verify the unlabelled manifest-structure counter series.

use super::ConfigMetricsRecorder;
use metrics::counter;
use metrics_util::{MetricKind, debugging::DebugValue};

/// Retain only the unlabelled manifest-structure counter series.
#[test]
fn recorder_retains_unlabelled_manifest_structure_counter() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        counter!("netsuke_runner_manifest_structures_total").increment(1);
        counter!("netsuke_runner_manifest_structures_total", "section" => "targets").increment(1);
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        1,
        "only the unlabelled manifest-structure counter series should be retained"
    );
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Counter
                && entry.0.key().name() == "netsuke_runner_manifest_structures_total"
                && entry.0.key().labels().next().is_none()
                && matches!(entry.3, DebugValue::Counter(1))
        }),
        "expected unlabelled manifest-structure counter: {snapshot:?}"
    );
}
