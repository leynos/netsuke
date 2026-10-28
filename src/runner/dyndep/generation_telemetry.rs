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
pub const BUNDLE_GENERATIONS_TOTAL: &str = "netsuke_ninja_dyndep_bundle_generations_total";
/// Metric name measuring runner-owned bundle generation duration in seconds.
pub const BUNDLE_GENERATION_DURATION: &str =
    "netsuke_ninja_dyndep_bundle_generation_duration_seconds";
/// Metric counting configured console edges across bundle-generation attempts.
pub const BUNDLE_CONSOLE_EDGES_TOTAL: &str = "netsuke_ninja_dyndep_bundle_console_edges_total";

/// Record one runner-owned bundle-generation operation.
pub(in crate::runner) fn instrument_bundle_generation<T>(
    graph: &BuildGraph,
    generate: impl FnOnce() -> Result<T, NinjaGenError>,
) -> Result<T, NinjaGenError> {
    describe_metrics();
    let dependency_count = graph
        .edges()
        .map(|edge| edge.implicit_deps.len())
        .sum::<usize>();
    // Count logical build operations once; `output_count` measures aliases and
    // would inflate this span for a single multi-output edge.
    let canonical_target_count = graph.edge_count();
    let console_edge_count = graph.edges().filter(|edge| edge.console).count();
    let console_scheduling = crate::runner::console_telemetry::scheduling_label(console_edge_count);
    let span = tracing::trace_span!(
        "runner.ninja.dyndep_bundle.generate",
        action_count = graph.actions.len(),
        target_count = canonical_target_count,
        dependency_count,
        console_edge_count,
        console_scheduling,
        outcome = field::Empty,
        error_category = field::Empty,
    );
    let _guard = span.enter();
    let started = Instant::now();
    let result = generate();
    let (outcome, error_category) = record_outcome(&span, &result);
    counter!(BUNDLE_GENERATIONS_TOTAL, "outcome" => outcome,
        "error_category" => error_category, "console_scheduling" => console_scheduling)
    .increment(1);
    histogram!(BUNDLE_GENERATION_DURATION, "outcome" => outcome,
        "error_category" => error_category, "console_scheduling" => console_scheduling)
    .record(started.elapsed());
    counter!(BUNDLE_CONSOLE_EDGES_TOTAL, "outcome" => outcome)
        .increment(u64::try_from(console_edge_count).unwrap_or(u64::MAX));
    result
}

/// Record the bounded outcome and error category on the span and return it.
fn record_outcome<T>(
    span: &tracing::Span,
    result: &Result<T, NinjaGenError>,
) -> (&'static str, &'static str) {
    match result {
        Ok(_) => {
            span.record("outcome", "success");
            span.record("error_category", "none");
            ("success", "none")
        }
        Err(error) => {
            let category = error_category(error);
            span.record("outcome", "error");
            span.record("error_category", category);
            tracing::debug!(error_category = category, "dyndep bundle generation failed");
            ("error", category)
        }
    }
}

/// Return the broad failure category for a Ninja-generation error.
const fn error_category(error: &NinjaGenError) -> &'static str {
    match error {
        NinjaGenError::MissingAction { .. } => "missing_action",
        NinjaGenError::ConsoleDependencyOnly { .. } => "console_dependency_only",
        NinjaGenError::ConsolePoolConflict { .. } => "console_pool_conflict",
        NinjaGenError::EmptyCommandRecipe { .. }
        | NinjaGenError::InvalidCommandIsolation { .. }
        | NinjaGenError::MultipleBackgroundJobs { .. }
        | NinjaGenError::UnsupportedCommandListExec { .. }
        | NinjaGenError::UnanalyzableCommandListEval { .. }
        | NinjaGenError::NinjaControlCharacter { .. } => "command_list",
        NinjaGenError::UnsafeNinjaValue => "unsafe_ninja_value",
        NinjaGenError::PowerShellRecipeTooLarge { .. }
        | NinjaGenError::PowerShellCommandLineTooLong { .. } => "power_shell_recipe_too_large",
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
            "Counts bundle-generation outcomes by bounded console-scheduling state and error category."
        );
        describe_histogram!(
            BUNDLE_GENERATION_DURATION,
            "Measures bundle-generation duration by bounded outcome, error category, and console-scheduling state."
        );
        describe_counter!(
            BUNDLE_CONSOLE_EDGES_TOTAL,
            "Counts configured console edges across runner-owned bundle-generation attempts."
        );
    });
}

#[cfg(test)]
mod tests {
    //! Tests for bounded generation telemetry at the runner boundary.

    use super::*;
    use crate::ir::{BuildEdge, DependencyOrder};
    use camino::Utf8PathBuf;
    use metrics_util::MetricKind;
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};
    use rstest::rstest;
    use std::sync::{Arc, Mutex, PoisonError};
    use tracing::{
        Subscriber,
        field::{Field, Visit},
        span::{Attributes, Id, Record},
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
        fn on_new_span(&self, values: &Attributes<'_>, _span: &Id, _ctx: LayerContext<'_, S>) {
            let mut visitor = SpanFieldVisitor::default();
            values.record(&mut visitor);
            self.fields
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend(visitor.fields);
        }

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
        test_support::tracing_capture::ensure_test_interest_anchors_registered();
        let captured = CapturedSpanFields::default();
        let subscriber =
            tracing_subscriber::registry().with(captured.clone().with_filter(LevelFilter::TRACE));
        tracing::subscriber::with_default(subscriber, || test(captured))
    }

    /// Represent one locally captured metric snapshot.
    type Snapshot = Vec<(
        metrics_util::CompositeKey,
        Option<metrics::Unit>,
        Option<metrics::SharedString>,
        DebugValue,
    )>;

    /// Find a metric with the exact finite label set.
    fn metric<'a>(
        snapshot: &'a Snapshot,
        name: &str,
        kind: MetricKind,
        labels: &[(&str, &str)],
    ) -> Option<&'a DebugValue> {
        snapshot.iter().find_map(|(key, _, _, value)| {
            let actual = key
                .key()
                .labels()
                .map(|label| (label.key(), label.value()))
                .collect::<Vec<_>>();
            (key.kind() == kind
                && key.key().name() == name
                && actual.len() == labels.len()
                && labels.iter().all(|label| actual.contains(label)))
            .then_some(value)
        })
    }

    /// Construct graph shape without invoking the generator under observation.
    fn graph_with_console_edges(console_edges: usize) -> BuildGraph {
        let mut graph = BuildGraph::default();
        for index in 0..=console_edges {
            graph
                .insert_edge(BuildEdge {
                    action_id: "shape-only".into(),
                    inputs: Vec::new(),
                    implicit_deps: Vec::new(),
                    dependency_order: DependencyOrder::Parallel,
                    explicit_outputs: vec![Utf8PathBuf::from(format!("private-{index}.out"))],
                    implicit_outputs: Vec::new(),
                    order_only_deps: Vec::new(),
                    phony: false,
                    always: false,
                    console: index < console_edges,
                })
                .expect("fixture aliases must be distinct");
        }
        graph
    }

    /// Build a controlled generation error with the requested fixed category.
    fn generation_error(category: &str) -> NinjaGenError {
        use crate::localization::{self, keys};
        match category {
            "command_list" => NinjaGenError::EmptyCommandRecipe { action_index: 1 },
            "console_dependency_only" => NinjaGenError::ConsoleDependencyOnly {
                output: "private-output".into(),
                message: localization::message(keys::NINJA_GEN_CONSOLE_DEPENDENCY_ONLY)
                    .with_arg("output", "private-output"),
            },
            "console_pool_conflict" => NinjaGenError::ConsolePoolConflict {
                output: "private-output".into(),
                pool: "private-pool".into(),
                message: localization::message(keys::NINJA_GEN_CONSOLE_POOL_CONFLICT)
                    .with_arg("output", "private-output")
                    .with_arg("pool", "private-pool"),
            },
            _ => panic!("fixture must name a known category"),
        }
    }

    /// Assert one bounded operation, one duration sample, and the configured-edge count.
    fn assert_metrics(snapshot: &Snapshot, console_edges: usize, labels: [(&str, &str); 3]) {
        assert_eq!(
            snapshot.len(),
            3,
            "only three bounded series are emitted: {snapshot:?}"
        );
        assert!(matches!(
            metric(
                snapshot,
                BUNDLE_GENERATIONS_TOTAL,
                MetricKind::Counter,
                &labels
            ),
            Some(DebugValue::Counter(1))
        ));
        assert!(
            matches!(metric(snapshot, BUNDLE_GENERATION_DURATION, MetricKind::Histogram, &labels), Some(DebugValue::Histogram(samples)) if samples.len() == 1)
        );
        let outcome = labels[0].1;
        assert!(
            matches!(metric(snapshot, BUNDLE_CONSOLE_EDGES_TOTAL, MetricKind::Counter, &[("outcome", outcome)]), Some(DebugValue::Counter(count)) if *count == u64::try_from(console_edges).expect("small fixture count"))
        );
    }

    #[rstest]
    #[case::ordinary_success(0, "disabled", "none")]
    #[case::console_success(2, "enabled", "none")]
    #[case::command_list_error(0, "disabled", "command_list")]
    #[case::dependency_only_error(2, "enabled", "console_dependency_only")]
    #[case::pool_conflict_error(2, "enabled", "console_pool_conflict")]
    fn generation_records_console_shape_and_bounded_outcomes(
        #[case] console_edges: usize,
        #[case] scheduling: &str,
        #[case] category: &str,
    ) {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        let graph = graph_with_console_edges(console_edges);
        let (result, fields) = with_span_capture(|captured| {
            let result = metrics::with_local_recorder(&recorder, || {
                instrument_bundle_generation(&graph, || {
                    if category == "none" {
                        Ok(())
                    } else {
                        Err(generation_error(category))
                    }
                })
            });
            (result, captured.snapshot())
        });
        let outcome = if category == "none" {
            "success"
        } else {
            "error"
        };
        assert_eq!(result.is_err(), category != "none");
        let snapshot = snapshotter.snapshot().into_vec();
        let labels = [
            ("outcome", outcome),
            ("error_category", category),
            ("console_scheduling", scheduling),
        ];
        assert_metrics(&snapshot, console_edges, labels);
        for expected in [
            format!("console_edge_count={console_edges}"),
            format!("console_scheduling={scheduling:?}"),
            format!("outcome={outcome:?}"),
            format!("error_category={category:?}"),
        ] {
            assert!(
                fields.contains(&expected),
                "span must record {expected}: {fields:?}"
            );
        }
        assert!(fields.iter().all(|field| !field.contains("private-")));
    }
}
