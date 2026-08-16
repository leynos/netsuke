//! Telemetry-contract tests for bounded dyndep retention.

use super::*;
use crate::runner::process::dyndep_telemetry::RETENTION_DURATION;
use metrics::{SharedString, Unit};
use metrics_util::{
    CompositeKey, MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use std::collections::BTreeMap;

type MetricSnapshotEntry = (CompositeKey, Option<Unit>, Option<SharedString>, DebugValue);

fn retention_outcome_counters(snapshot: &[MetricSnapshotEntry]) -> BTreeMap<Option<&str>, u64> {
    let mut counters = BTreeMap::new();
    for (key, _, _, debug_value) in snapshot {
        if let (MetricKind::Counter, name, DebugValue::Counter(count)) =
            (key.kind(), key.key().name(), debug_value)
            && name == RETENTIONS_TOTAL
        {
            let outcome = key
                .key()
                .labels()
                .find(|label| label.key() == "outcome")
                .map(metrics::Label::value);
            *counters.entry(outcome).or_default() += count;
        }
    }
    counters
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

fn retention_duration_sample_count(snapshot: &[MetricSnapshotEntry]) -> usize {
    snapshot
        .iter()
        .find_map(
            |(key, _, _, debug_value)| match (key.kind(), key.key().name(), debug_value) {
                (MetricKind::Histogram, name, DebugValue::Histogram(samples))
                    if name == RETENTION_DURATION =>
                {
                    Some(samples.len())
                }
                _ => None,
            },
        )
        .unwrap_or_default()
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
        retention_outcome_counters(&snapshot) == BTreeMap::from([(Some("success"), 1)]),
        "retention must record exactly one success retention outcome"
    );
    ensure!(
        reclaimed_counter_value(&snapshot, RETAINED_FILES_RECLAIMED) == Some(1),
        "retention must record reclaimed sidecar files"
    );
    ensure!(
        reclaimed_counter_value(&snapshot, RETAINED_BYTES_RECLAIMED) == Some(5),
        "retention must record reclaimed sidecar bytes"
    );
    ensure!(
        retention_duration_sample_count(&snapshot) == 1,
        "retention must record one duration sample"
    );
    Ok(())
}
