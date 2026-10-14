//! Record bounded observability for manifest-to-IR graph generation.
//!
//! This runner boundary owns telemetry for graph construction, including
//! command-interpolation failures. It records only the selected shell and
//! fixed outcome categories, never manifest text, paths, or recipe contents.

use std::sync::Once;

use metrics::{counter, describe_counter, describe_histogram, histogram};
use monotony::MonotonicClock;
use tracing::field;

use crate::{ir::IrGenError, recipe_shell::RecipeShell};

/// Metric name counting runner-owned IR graph-generation outcomes.
const GRAPH_GENERATIONS_TOTAL: &str = "netsuke_runner_graph_generations_total";
/// Metric name measuring runner-owned IR graph-generation duration in seconds.
const GRAPH_GENERATION_DURATION: &str = "netsuke_runner_graph_generation_duration_seconds";

/// Instrument one manifest-to-IR graph-generation operation.
pub(super) fn instrument_graph_generation<T, Clock>(
    clock: &Clock,
    shell: RecipeShell,
    generate: impl FnOnce() -> Result<T, IrGenError>,
) -> Result<T, IrGenError>
where
    Clock: MonotonicClock + ?Sized,
{
    describe_metrics();
    let recipe_shell = shell_name(shell);
    let span = tracing::trace_span!(
        "runner.ir.graph.generate",
        recipe_shell,
        outcome = field::Empty,
        error_category = field::Empty,
    );
    let _guard = span.enter();
    let started = clock.now();
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
    .record(clock.now().duration_since(started));
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
    use monotony::test_util::FixedMonotonicClock;
    use std::time::Duration;

    /// Build one interpolation failure without exposing its command contents to metrics.
    fn invalid_command_error() -> IrGenError {
        IrGenError::InvalidCommand {
            command: "echo {{ ins }}".into(),
            snippet: "echo {{ ins }}".into(),
            message: localization::message(keys::IR_INVALID_COMMAND)
                .with_arg("snippet", "echo {{ ins }}"),
        }
    }

    /// Build a non-interpolation graph-generation failure for label coverage.
    fn invalid_manifest_error() -> IrGenError {
        IrGenError::InvalidManifest {
            message: "invalid manifest",
        }
    }

    /// Instrument one graph-generation operation with a deterministic clock.
    fn instrument_with_fixed_clock<T>(
        shell: RecipeShell,
        generate: impl FnOnce() -> Result<T, IrGenError>,
    ) -> Result<T, IrGenError> {
        let clock = FixedMonotonicClock::with_elapsed(Duration::ZERO);
        instrument_graph_generation(&clock, shell, generate)
    }

    /// Report whether one metric key has the expected bounded graph-generation labels.
    fn has_graph_generation_labels(
        key: &metrics_util::CompositeKey,
        outcome: &str,
        error_category: &str,
        recipe_shell: &str,
    ) -> bool {
        [
            ("outcome", outcome),
            ("error_category", error_category),
            ("recipe_shell", recipe_shell),
        ]
        .into_iter()
        .all(|(name, value)| {
            key.key()
                .labels()
                .any(|label| label.key() == name && label.value() == value)
        })
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
        recipe_shell: &str,
    ) -> Option<u64> {
        snapshot.iter().find_map(
            |(key, _, _, value)| match (key.kind(), key.key().name(), value) {
                (MetricKind::Counter, GRAPH_GENERATIONS_TOTAL, DebugValue::Counter(count))
                    if has_graph_generation_labels(key, outcome, error_category, recipe_shell) =>
                {
                    Some(*count)
                }
                _ => None,
            },
        )
    }

    #[test]
    fn graph_generation_records_bounded_outcome_labels() {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        metrics::with_local_recorder(&recorder, || {
            instrument_with_fixed_clock(RecipeShell::Posix, || Ok::<_, IrGenError>(()))
                .expect("successful graph generation should be preserved");
            let error = instrument_with_fixed_clock(RecipeShell::Posix, || {
                Err::<(), _>(invalid_command_error())
            })
            .expect_err("interpolation failure should be preserved");
            assert!(matches!(error, IrGenError::InvalidCommand { .. }));
            instrument_with_fixed_clock(RecipeShell::Bash, || Ok::<_, IrGenError>(()))
                .expect("Bash graph generation should be preserved");
            let non_interpolation_error =
                instrument_with_fixed_clock(RecipeShell::PowerShell, || {
                    Err::<(), _>(invalid_manifest_error())
                })
                .expect_err("non-interpolation failure should be preserved");
            assert!(matches!(
                non_interpolation_error,
                IrGenError::InvalidManifest { .. }
            ));
        });

        let snapshot = snapshotter.snapshot().into_vec();
        assert_eq!(
            graph_generation_count(&snapshot, "success", "none", "posix"),
            Some(1)
        );
        assert_eq!(
            graph_generation_count(&snapshot, "error", "invalid_command_interpolation", "posix"),
            Some(1)
        );
        assert_eq!(
            graph_generation_count(&snapshot, "success", "none", "bash"),
            Some(1)
        );
        assert_eq!(
            graph_generation_count(&snapshot, "error", "other", "powershell"),
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
        assert_eq!(duration_samples, 4);
    }

    /// Verify telemetry helpers expose only bounded shell and error labels.
    #[test]
    fn graph_generation_telemetry_labels_are_bounded() {
        assert_eq!(shell_name(RecipeShell::Posix), "posix");
        assert_eq!(shell_name(RecipeShell::Bash), "bash");
        assert_eq!(shell_name(RecipeShell::PowerShell), "powershell");
        assert_eq!(
            error_category(&invalid_command_error()),
            "invalid_command_interpolation"
        );
        assert_eq!(error_category(&invalid_manifest_error()), "other");
    }
}
