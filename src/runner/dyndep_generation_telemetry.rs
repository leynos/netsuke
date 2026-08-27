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

/// Metric name counting runner-owned bundle generation outcomes.
const BUNDLE_GENERATIONS_TOTAL: &str = "netsuke_ninja_dyndep_bundle_generations_total";
/// Metric name measuring runner-owned bundle generation duration in seconds.
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

/// Record the bounded outcome and error category on the span and return it.
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

/// Return the broad failure category for a Ninja-generation error.
const fn error_category(error: &NinjaGenError) -> &'static str {
    match error {
        NinjaGenError::MissingAction { .. } => "missing_action",
        NinjaGenError::EmptyCommandRecipe { .. }
        | NinjaGenError::MultipleBackgroundJobs { .. }
        | NinjaGenError::UnsupportedCommandListExec { .. }
        | NinjaGenError::UnanalyzableCommandListEval { .. }
        | NinjaGenError::NinjaControlCharacter { .. } => "command_list",
        NinjaGenError::UnsafeNinjaValue => "unsafe_ninja_value",
        NinjaGenError::Format { .. } => "format",
        NinjaGenError::DyndepFilesRequired { .. } => "dyndep_files_required",
        NinjaGenError::ReservedOutputPath { .. } => "reserved_output_path",
        NinjaGenError::UnsupportedPathCharacter { .. } => "unsupported_path_character",
        NinjaGenError::UnsafeNinjaPath { .. } => "unsafe_ninja_path",
    }
}

/// Register the bundle-generation metric descriptions once per process.
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
    use std::sync::{Arc, Mutex, PoisonError};
    use tracing::{
        Subscriber,
        field::{Field, Visit},
        span::{Id, Record},
    };
    use tracing_subscriber::{
        Layer, filter::LevelFilter, layer::Context as LayerContext, prelude::*,
        registry::LookupSpan,
    };

    #[derive(Clone, Default)]
    struct CapturedSpanFields {
        fields: Arc<Mutex<Vec<String>>>,
    }

    impl CapturedSpanFields {
        fn snapshot(&self) -> Vec<String> {
            self.fields
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }
    }

    impl<S> Layer<S> for CapturedSpanFields
    where
        S: Subscriber + for<'span> LookupSpan<'span>,
    {
        fn on_record(&self, _span: &Id, values: &Record<'_>, _ctx: LayerContext<'_, S>) {
            let mut visitor = SpanFieldVisitor::default();
            values.record(&mut visitor);
            self.fields
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend(visitor.fields);
        }
    }

    #[derive(Default)]
    struct SpanFieldVisitor {
        fields: Vec<String>,
    }

    impl Visit for SpanFieldVisitor {
        fn record_str(&mut self, field: &Field, value: &str) {
            self.fields.push(format!("{}={value:?}", field.name()));
        }

        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            self.fields.push(format!("{}={value:?}", field.name()));
        }
    }

    fn with_span_capture<T>(test: impl FnOnce(CapturedSpanFields) -> T) -> T {
        let captured = CapturedSpanFields::default();
        let subscriber =
            tracing_subscriber::registry().with(captured.clone().with_filter(LevelFilter::TRACE));
        tracing::subscriber::with_default(subscriber, || test(captured))
    }

    fn bundle_generation_count(
        snapshot: &[(
            metrics_util::CompositeKey,
            Option<metrics::Unit>,
            Option<metrics::SharedString>,
            DebugValue,
        )],
        outcome: &str,
    ) -> Option<u64> {
        snapshot.iter().find_map(|(key, _, _, value)| {
            let has_outcome = key
                .key()
                .labels()
                .any(|label| label.key() == "outcome" && label.value() == outcome);
            match (key.kind(), key.key().name(), value) {
                (MetricKind::Counter, BUNDLE_GENERATIONS_TOTAL, DebugValue::Counter(count))
                    if has_outcome =>
                {
                    Some(*count)
                }
                _ => None,
            }
        })
    }

    fn record_generation<T>(
        recorder: &DebuggingRecorder,
        graph: &BuildGraph,
        generate: impl FnOnce() -> Result<T, NinjaGenError>,
    ) -> Result<T, NinjaGenError> {
        metrics::with_local_recorder(recorder, || instrument_bundle_generation(graph, generate))
    }

    #[test]
    fn runner_boundary_records_bundle_generation() {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        let graph = BuildGraph::default();
        let (success, error, span_fields) = with_span_capture(|captured| {
            let success = record_generation(&recorder, &graph, || Ok::<_, NinjaGenError>(()));
            let error = record_generation(&recorder, &graph, || {
                Err::<(), _>(NinjaGenError::EmptyCommandRecipe { action_index: 1 })
            });
            (success, error, captured.snapshot())
        });
        assert!(success.is_ok());
        assert!(matches!(
            error,
            Err(NinjaGenError::EmptyCommandRecipe { action_index: 1 })
        ));
        let snapshot = snapshotter.snapshot().into_vec();
        assert_eq!(bundle_generation_count(&snapshot, "success"), Some(1));
        assert_eq!(bundle_generation_count(&snapshot, "error"), Some(1));
        assert!(snapshot.iter().any(|(key, _, _, value)| {
            key.kind() == MetricKind::Histogram
                && key.key().name() == BUNDLE_GENERATION_DURATION
                && matches!(value, DebugValue::Histogram(samples) if samples.len() == 2)
        }));
        assert!(
            span_fields
                .iter()
                .any(|field| field == "error_category=\"command_list\""),
            "the error span must record the bounded command-list category: {span_fields:?}"
        );
    }
}
