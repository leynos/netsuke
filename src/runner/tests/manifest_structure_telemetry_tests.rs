//! Integration coverage for runner-owned manifest-structure telemetry.

use super::{GraphGenerationFixture, generate_ninja_with_shell, graph_generation_fixture};
use crate::test_tracing_capture::with_test_subscriber;
use anyhow::{Result, ensure};
use metrics_util::{
    MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use rstest::rstest;
use tracing_subscriber::filter::LevelFilter;

/// Verify runner generation records one unlabelled manifest-structure summary.
#[rstest]
fn runner_generation_records_one_unlabelled_manifest_structure_counter(
    graph_generation_fixture: Result<GraphGenerationFixture>,
) -> Result<()> {
    let fixture = graph_generation_fixture?;
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let (generation, events) = with_test_subscriber(LevelFilter::TRACE, |captured| {
        let generation = metrics::with_local_recorder(&recorder, || {
            let graph_generation = fixture.graph_generation_context();
            generate_ninja_with_shell(&fixture.cli, &fixture.reporter, None, &graph_generation)
        });
        (generation, captured.snapshot())
    });
    generation?;

    ensure!(
        events
            .iter()
            .any(|event| event.contains("message=manifest structure summary")),
        "runner generation should emit the manifest-structure trace event"
    );

    let summaries = snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .filter(|entry| {
            entry.0.kind() == MetricKind::Counter
                && entry.0.key().name() == "netsuke_runner_manifest_structures_total"
        })
        .collect::<Vec<_>>();
    ensure!(
        summaries.len() == 1,
        "runner generation must record exactly one manifest-structure counter series"
    );
    ensure!(
        summaries.iter().any(|entry| {
            entry.0.key().labels().next().is_none() && matches!(entry.3, DebugValue::Counter(1))
        }),
        "runner generation must record one unlabelled manifest-structure counter"
    );
    Ok(())
}
