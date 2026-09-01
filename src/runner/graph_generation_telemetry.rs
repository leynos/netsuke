//! Record bounded observability for manifest-to-IR graph generation.
//!
//! This runner boundary owns telemetry for graph construction, including
//! command-interpolation failures. It records only the selected shell and
//! fixed outcome categories, never manifest text, paths, or recipe contents.

use std::{sync::Once, time::Instant};

use metrics::{counter, describe_counter, describe_histogram, histogram};
use tracing::field;

use crate::{ir::IrGenError, recipe_shell::RecipeShell};

/// Metric name counting runner-owned IR graph-generation outcomes.
const GRAPH_GENERATIONS_TOTAL: &str = "netsuke_runner_graph_generations_total";
/// Metric name measuring runner-owned IR graph-generation duration in seconds.
const GRAPH_GENERATION_DURATION: &str = "netsuke_runner_graph_generation_duration_seconds";

/// Instrument one manifest-to-IR graph-generation operation.
pub(super) fn instrument_graph_generation<T>(
    shell: RecipeShell,
    generate: impl FnOnce() -> Result<T, IrGenError>,
) -> Result<T, IrGenError> {
    describe_metrics();
    let recipe_shell = shell_name(shell);
    let span = tracing::trace_span!(
        "runner.ir.graph.generate",
        recipe_shell,
        outcome = field::Empty,
        error_category = field::Empty,
    );
    let _guard = span.enter();
    let started = Instant::now();
    let result = generate();
    let (outcome, error_category) = record_outcome(&span, &result);
    counter!(
        GRAPH_GENERATIONS_TOTAL,
        "outcome" => outcome,
        "error_category" => error_category,
        "recipe_shell" => recipe_shell,
    )
    .increment(1);
    histogram!(
        GRAPH_GENERATION_DURATION,
        "outcome" => outcome,
        "error_category" => error_category,
        "recipe_shell" => recipe_shell,
    )
    .record(started.elapsed());
    result
}

/// Record a bounded result on `span` and return its metric labels.
fn record_outcome<T>(
    span: &tracing::Span,
    result: &Result<T, IrGenError>,
) -> (&'static str, &'static str) {
    match result {
        Ok(_) => {
            span.record("outcome", "success");
            ("success", "none")
        }
        Err(error) => {
            let category = error_category(error);
            span.record("outcome", "error");
            span.record("error_category", category);
            tracing::debug!(error_category = category, "IR graph generation failed");
            ("error", category)
        }
    }
}

/// Return the fixed category exposed for one graph-generation failure.
const fn error_category(error: &IrGenError) -> &'static str {
    match error {
        IrGenError::InvalidCommand { .. } => "invalid_command_interpolation",
        _ => "other",
    }
}

/// Return the bounded metric label for one selected recipe shell.
const fn shell_name(shell: RecipeShell) -> &'static str {
    match shell {
        RecipeShell::Posix => "posix",
        RecipeShell::PowerShell => "powershell",
        RecipeShell::Bash => "bash",
    }
}

/// Register graph-generation metric descriptions once per process.
fn describe_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            GRAPH_GENERATIONS_TOTAL,
            "Counts runner-owned graph-generation outcomes by bounded shell and error category."
        );
        describe_histogram!(
            GRAPH_GENERATION_DURATION,
            "Measures runner-owned graph-generation duration in seconds by bounded outcome."
        );
    });
}

#[cfg(test)]
mod tests {
    //! Tests for bounded graph-generation telemetry at the runner boundary.

    use super::*;
    use crate::localization::{self, keys};
    use metrics_util::{
        MetricKind,
        debugging::{DebugValue, DebuggingRecorder},
    };

    /// Build one interpolation failure without exposing its command contents to metrics.
    fn invalid_command_error() -> IrGenError {
        IrGenError::InvalidCommand {
            command: "echo {{ ins }}".into(),
            snippet: "echo {{ ins }}".into(),
            message: localization::message(keys::IR_INVALID_COMMAND)
                .with_arg("snippet", "echo {{ ins }}"),
        }
    }

    /// Locate one graph-generation counter with the supplied bounded labels.
    fn graph_generation_count(
        snapshot: &[(
            metrics_util::CompositeKey,
            Option<metrics::Unit>,
            Option<metrics::SharedString>,
            DebugValue,
        )],
        outcome: &str,
        error_category: &str,
    ) -> Option<u64> {
        snapshot.iter().find_map(|(key, _, _, value)| {
            let has_outcome = key
                .key()
                .labels()
                .any(|label| label.key() == "outcome" && label.value() == outcome);
            let has_category = key
                .key()
                .labels()
                .any(|label| label.key() == "error_category" && label.value() == error_category);
            match (key.kind(), key.key().name(), value) {
                (MetricKind::Counter, GRAPH_GENERATIONS_TOTAL, DebugValue::Counter(count))
                    if has_outcome && has_category =>
                {
                    Some(*count)
                }
                _ => None,
            }
        })
    }

    #[test]
    fn graph_generation_records_success_and_interpolation_failure() {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        metrics::with_local_recorder(&recorder, || {
            instrument_graph_generation(RecipeShell::Posix, || Ok::<_, IrGenError>(()))
                .expect("successful graph generation should be preserved");
            let error = instrument_graph_generation(RecipeShell::Posix, || {
                Err::<(), _>(invalid_command_error())
            })
            .expect_err("interpolation failure should be preserved");
            assert!(matches!(error, IrGenError::InvalidCommand { .. }));
        });

        let snapshot = snapshotter.snapshot().into_vec();
        assert_eq!(
            graph_generation_count(&snapshot, "success", "none"),
            Some(1)
        );
        assert_eq!(
            graph_generation_count(&snapshot, "error", "invalid_command_interpolation"),
            Some(1)
        );
        let duration_samples = snapshot
            .iter()
            .filter(|(key, _, _, _)| {
                key.kind() == MetricKind::Histogram && key.key().name() == GRAPH_GENERATION_DURATION
            })
            .map(|(_, _, _, value)| match value {
                DebugValue::Histogram(samples) => samples.len(),
                _ => 0,
            })
            .sum::<usize>();
        assert_eq!(duration_samples, 2);
    }
}
