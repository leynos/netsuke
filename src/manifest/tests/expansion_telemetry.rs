//! Telemetry coverage for expansion reports emitted by manifest loading.

use super::super::from_str;
use crate::test_tracing_capture::with_test_subscriber;
use anyhow::{Context, Result, bail, ensure};
use metrics::SharedString;
use metrics_util::{
    CompositeKey, MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use test_support::manifest::manifest_yaml;
use tracing::level_filters::LevelFilter;

const FILTERED_TARGETS_TOTAL: &str = "netsuke_manifest_filtered_targets_total";
const FILTERED_ACTIONS_TOTAL: &str = "netsuke_manifest_filtered_actions_total";
const OMITTED_FILTERED_ENTRIES_TOTAL: &str = "netsuke_manifest_omitted_filtered_entries_total";

type Snapshot = Vec<(
    CompositeKey,
    Option<metrics::Unit>,
    Option<SharedString>,
    DebugValue,
)>;

/// Assert that an unlabeled counter has exactly one series with `expected`.
fn assert_unlabeled_counter(snapshot: &Snapshot, name: &str, expected: u64) -> Result<()> {
    let series: Vec<_> = snapshot
        .iter()
        .filter(|(key, _, _, _)| key.kind() == MetricKind::Counter && key.key().name() == name)
        .collect();
    ensure!(
        series.len() == 1,
        "expected exactly one counter series for {name}: {snapshot:?}"
    );
    let [(key, _, _, DebugValue::Counter(value))] = series.as_slice() else {
        bail!("expected a counter value for {name}: {snapshot:?}");
    };
    ensure!(
        key.key().labels().next().is_none(),
        "counter {name} must not have labels: {key:?}"
    );
    ensure!(
        *value == expected,
        "unexpected counter value for {name}: {value}"
    );
    Ok(())
}

/// Verify that manifest loading emits bounded filtering telemetry.
#[test]
fn manifest_loading_traces_filtered_entries_and_summary() -> Result<()> {
    let filtered_count = 65;
    let foreach_values = (0..filtered_count)
        .map(|index| format!("      - {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    let yaml = manifest_yaml(&format!(
        "targets:\n  - name: skipped-target\n    command: echo {{{{ item }}}}\n    foreach:\n{foreach_values}\n    when: 'false'"
    ));

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || {
        with_test_subscriber(LevelFilter::DEBUG, |captured| {
            let manifest = from_str(&yaml)?;
            let events = captured.snapshot();

            ensure!(
                manifest.targets.is_empty(),
                "filtered target must be removed"
            );
            let retained_events: Vec<_> = events
                .iter()
                .filter(|event| event.contains("filtered manifest entry by when expression"))
                .collect();
            ensure!(
                retained_events.len() == 64,
                "loading must emit at most 64 retained filtering events: {events:?}"
            );
            ensure!(
                retained_events.iter().all(|event| {
                    event.contains("section=\"targets\"")
                        && event.contains("when_expression_len=5")
                        && event.contains("when_result=false")
                }),
                "retained events must preserve bounded metadata: {retained_events:?}"
            );
            let summary = events
                .iter()
                .find(|event| event.contains("expanded manifest foreach and when directives"))
                .context("missing expansion summary event")?;
            ensure!(
                summary.contains("filtered_targets=65")
                    && summary.contains("filtered_actions=0")
                    && summary.contains("filtered_entry_count=65")
                    && summary.contains("omitted_filtered_entries=1"),
                "summary must report exact aggregate filtering counts: {summary}"
            );
            ensure!(
                events.iter().all(|event| !event.contains("skipped-target")
                    && !event.contains("when_expression=")),
                "telemetry must not disclose raw filtering inputs: {events:?}"
            );
            Ok(events)
        })
    })?;
    let snapshot = snapshotter.snapshot().into_vec();
    assert_unlabeled_counter(&snapshot, FILTERED_TARGETS_TOTAL, 65)?;
    assert_unlabeled_counter(&snapshot, FILTERED_ACTIONS_TOTAL, 0)?;
    assert_unlabeled_counter(&snapshot, OMITTED_FILTERED_ENTRIES_TOTAL, 1)?;
    Ok(())
}
