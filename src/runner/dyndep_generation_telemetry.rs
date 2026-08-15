//! Runner-boundary observability for staged dyndep bundle generation.
//!
//! This module owns timing, tracing, and metric registration around the pure
//! Ninja generator query. It accepts only aggregate graph-shape counts and
//! emits fixed outcome and error-category values; paths, identifiers,
//! manifests, and generated content never cross the telemetry boundary.

use crate::ir::BuildGraph;
use crate::ninja_gen::NinjaGenError;
use metrics::{counter, describe_counter, describe_histogram, histogram};
use std::{sync::Once, time::Instant};
use tracing::field;

const BUNDLE_GENERATIONS_TOTAL: &str = "netsuke_ninja_dyndep_bundle_generations_total";
const BUNDLE_GENERATION_DURATION: &str = "netsuke_ninja_dyndep_bundle_generation_duration_seconds";

/// Record one runner-owned bundle-generation operation.
pub(super) fn instrument_bundle_generation<T>(
    graph: &BuildGraph,
    generate: impl FnOnce() -> Result<T, NinjaGenError>,
) -> Result<T, NinjaGenError> {
    describe_metrics();
    let dependency_count = graph
        .targets
        .values()
        .map(|edge| edge.implicit_deps.len())
        .sum::<usize>();
    let span = tracing::trace_span!(
        "runner.ninja.dyndep_bundle.generate",
        action_count = graph.actions.len(),
        target_count = graph.targets.len(),
        dependency_count,
        outcome = field::Empty,
        error_category = field::Empty,
    );
    let _guard = span.enter();
    let started = Instant::now();
    let result = generate();
    let outcome = record_outcome(&span, &result);
    counter!(BUNDLE_GENERATIONS_TOTAL, "outcome" => outcome).increment(1);
    histogram!(BUNDLE_GENERATION_DURATION).record(started.elapsed());
    result
}

fn record_outcome<T>(span: &tracing::Span, result: &Result<T, NinjaGenError>) -> &'static str {
    match result {
        Ok(_) => {
            span.record("outcome", "success");
            "success"
        }
        Err(error) => {
            let category = error_category(error);
            span.record("outcome", "error");
            span.record("error_category", category);
            tracing::debug!(error_category = category, "dyndep bundle generation failed");
            "error"
        }
    }
}

const fn error_category(error: &NinjaGenError) -> &'static str {
    match error {
        NinjaGenError::MissingAction { .. } => "missing_action",
        NinjaGenError::EmptyCommandRecipe { .. }
        | NinjaGenError::MultipleBackgroundJobs { .. }
        | NinjaGenError::UnsupportedCommandListExec { .. }
        | NinjaGenError::UnanalyzableCommandListEval { .. }
        | NinjaGenError::NinjaControlCharacter { .. } => "command_list",
        NinjaGenError::Format { .. } => "format",
        NinjaGenError::DyndepFilesRequired { .. } => "dyndep_files_required",
        NinjaGenError::ReservedOutputPath { .. } => "reserved_output_path",
        NinjaGenError::UnsupportedPathCharacter { .. } => "unsupported_path_character",
    }
}

fn describe_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            BUNDLE_GENERATIONS_TOTAL,
            "Counts runner-owned dyndep bundle generation outcomes."
        );
        describe_histogram!(
            BUNDLE_GENERATION_DURATION,
            "Measures runner-owned dyndep bundle generation duration in seconds."
        );
    });
}

#[cfg(test)]
mod tests {
    //! Tests for bounded generation telemetry at the runner boundary.

    use super::*;
    use metrics_util::MetricKind;
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    #[test]
    fn runner_boundary_records_bundle_generation() {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        let graph = BuildGraph::default();
        let result = metrics::with_local_recorder(&recorder, || {
            instrument_bundle_generation(&graph, || Ok::<_, NinjaGenError>(()))
        });
        assert!(result.is_ok());
        let snapshot = snapshotter.snapshot().into_vec();
        assert!(snapshot.iter().any(|(key, _, _, value)| {
            key.kind() == MetricKind::Counter
                && key.key().name() == BUNDLE_GENERATIONS_TOTAL
                && matches!(value, DebugValue::Counter(1))
        }));
        assert!(snapshot.iter().any(|(key, _, _, value)| {
            key.kind() == MetricKind::Histogram
                && key.key().name() == BUNDLE_GENERATION_DURATION
                && matches!(value, DebugValue::Histogram(samples) if samples.len() == 1)
        }));
    }
}
