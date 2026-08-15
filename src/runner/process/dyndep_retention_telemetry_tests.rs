//! Telemetry-contract tests for bounded dyndep retention.

use super::*;
use metrics::{SharedString, Unit};
use metrics_util::{
    CompositeKey, MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};

type MetricSnapshotEntry = (CompositeKey, Option<Unit>, Option<SharedString>, DebugValue);

fn successful_retention_count(snapshot: &[MetricSnapshotEntry]) -> Option<u64> {
    snapshot.iter().find_map(|(key, _, _, debug_value)| {
        let is_success = key
            .key()
            .labels()
            .any(|label| label.key() == "outcome" && label.value() == "success");
        match (key.kind(), key.key().name(), is_success, debug_value) {
            (MetricKind::Counter, name, true, DebugValue::Counter(count))
                if name == RETENTIONS_TOTAL =>
            {
                Some(*count)
            }
            _ => None,
        }
    })
}

fn reclaimed_counter_value(snapshot: &[MetricSnapshotEntry], name: &str) -> Option<u64> {
    snapshot.iter().find_map(|(key, _, _, debug_value)| {
        match (key.kind(), key.key().name(), debug_value) {
            (MetricKind::Counter, metric, DebugValue::Counter(count)) if metric == name => {
                Some(*count)
            }
            _ => None,
        }
    })
}

#[test]
fn retention_records_only_a_bounded_success_outcome_and_reclaimed_totals() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temporary_dir(&temp)?;
    let current = sidecar(".netsuke/dyndep/current.dd", "current");
    let lease = materialize_dyndep_files(&dir, std::slice::from_ref(&current))?;
    dir.write(".netsuke/dyndep/stale.dd", "stale")?;
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        prune_dyndep_sidecars(
            &dir,
            &lease,
            std::slice::from_ref(&current),
            RetentionPolicy::new(0, 0),
        )
    })?;

    let snapshot = snapshotter.snapshot().into_vec();
    ensure!(
        successful_retention_count(&snapshot) == Some(1),
        "retention must record exactly one fixed success outcome"
    );
    ensure!(
        reclaimed_counter_value(&snapshot, RETAINED_FILES_RECLAIMED) == Some(1),
        "retention must record reclaimed sidecar files"
    );
    ensure!(
        reclaimed_counter_value(&snapshot, RETAINED_BYTES_RECLAIMED) == Some(5),
        "retention must record reclaimed sidecar bytes"
    );
    Ok(())
}
