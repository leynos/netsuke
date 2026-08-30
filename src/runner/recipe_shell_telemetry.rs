//! Bounded observability for legacy recipe-shell resolution and preflight.
//!
//! Recipe text, environment values, command lines, paths, Bash output, and
//! process statuses are all user-controlled or unbounded. This module records
//! only the selected interpreter and fixed result categories at the runner
//! boundary.

use crate::recipe_shell::RecipeShell;
use anyhow::Result;
use metrics::{counter, describe_counter};
use std::sync::Once;
use tracing::{field, info};

/// Count recipe-shell resolution outcomes by bounded interpreter and category.
pub const RECIPE_SHELL_RESOLUTIONS_TOTAL: &str = "netsuke_runner_recipe_shell_resolutions_total";
/// Count Bash compatibility preflight outcomes by bounded probe result.
pub const BASH_PREFLIGHT_TOTAL: &str = "netsuke_runner_recipe_shell_bash_preflight_total";

/// Classify an explicit Bash runtime probe without exposing process details.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BashProbeOutcome {
    /// Record a runtime that accepted the version probe.
    Success,
    /// Record a runtime absent from the executable search path.
    NotFound,
    /// Record a process-start error other than a missing executable.
    LaunchFailed,
    /// Record a runtime that rejected the version probe with a non-zero exit.
    NonZeroExit,
}

impl BashProbeOutcome {
    /// Return the fixed telemetry label for this probe result.
    const fn label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::NotFound => "not_found",
            Self::LaunchFailed => "launch_failed",
            Self::NonZeroExit => "non_zero_exit",
        }
    }
}

/// Record the bounded outcome of resolving one legacy recipe shell.
pub(super) fn instrument_recipe_shell_resolution(
    resolve: impl FnOnce() -> Result<RecipeShell>,
) -> Result<RecipeShell> {
    describe_metrics();
    let span = tracing::info_span!(
        "runner.recipe_shell.resolve",
        recipe_shell = field::Empty,
        outcome = field::Empty,
        error_category = field::Empty,
    );
    let _guard = span.enter();
    let result = resolve();
    let (shell, outcome, error_category) = result.as_ref().map_or_else(
        |_| {
            (
                shell_label(RecipeShell::host_default()),
                "error",
                "invalid_selection",
            )
        },
        |shell| (shell_label(*shell), "success", "none"),
    );
    span.record("recipe_shell", shell);
    span.record("outcome", outcome);
    span.record("error_category", error_category);
    info!(
        recipe_shell = shell,
        outcome, error_category, "Resolved recipe shell"
    );
    counter!(
        RECIPE_SHELL_RESOLUTIONS_TOTAL,
        "recipe_shell" => shell,
        "outcome" => outcome,
        "error_category" => error_category,
    )
    .increment(1);
    result
}

/// Record the bounded result of one explicitly selected Bash compatibility probe.
pub(super) fn instrument_bash_preflight<T>(
    probe_outcome: BashProbeOutcome,
    preflight: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let probe_outcome_label = probe_outcome.label();
    describe_metrics();
    let span = tracing::info_span!(
        "runner.recipe_shell.bash_preflight",
        recipe_shell = "bash",
        outcome = field::Empty,
        probe_outcome = probe_outcome_label,
    );
    let _guard = span.enter();
    let result = preflight();
    let outcome = if result.is_ok() { "success" } else { "error" };
    span.record("outcome", outcome);
    info!(
        recipe_shell = "bash",
        outcome,
        probe_outcome = probe_outcome_label,
        "Completed Bash preflight"
    );
    counter!(
        BASH_PREFLIGHT_TOTAL,
        "recipe_shell" => "bash",
        "outcome" => outcome,
        "probe_outcome" => probe_outcome_label,
    )
    .increment(1);
    result
}

/// Map one recipe-shell variant onto its fixed telemetry label.
const fn shell_label(shell: RecipeShell) -> &'static str {
    match shell {
        RecipeShell::Posix => "posix",
        RecipeShell::PowerShell => "powershell",
        RecipeShell::Bash => "bash",
    }
}

/// Describe the stable bounded recipe-shell metrics once per process.
fn describe_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            RECIPE_SHELL_RESOLUTIONS_TOTAL,
            "Counts legacy recipe-shell resolution outcomes by bounded labels."
        );
        describe_counter!(
            BASH_PREFLIGHT_TOTAL,
            "Counts explicit Bash compatibility preflight outcomes by bounded labels."
        );
    });
}

#[cfg(test)]
mod tests {
    //! Verifies the fixed recipe-shell telemetry labels and tracing fields.

    use super::{BASH_PREFLIGHT_TOTAL, RECIPE_SHELL_RESOLUTIONS_TOTAL, *};
    use crate::test_tracing_capture::with_test_subscriber;
    use anyhow::bail;
    use metrics_util::{
        CompositeKey, MetricKind,
        debugging::{DebugValue, DebuggingRecorder},
    };
    use tracing_subscriber::filter::LevelFilter;

    /// Represent one drained local metrics snapshot.
    type Snapshot = Vec<(
        CompositeKey,
        Option<metrics::Unit>,
        Option<metrics::SharedString>,
        DebugValue,
    )>;

    /// Record one operation under isolated metrics and tracing capture.
    fn record<T>(operation: impl FnOnce() -> T) -> (T, Snapshot, Vec<String>) {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        let (result, events) = metrics::with_local_recorder(&recorder, || {
            with_test_subscriber(LevelFilter::INFO, |captured| {
                let result = operation();
                (result, captured.snapshot())
            })
        });
        (result, snapshotter.snapshot().into_vec(), events)
    }

    /// Assert that `snapshot` records one counter with exactly `labels`.
    fn assert_counter(snapshot: &Snapshot, name: &str, labels: &[(&str, &str)]) {
        assert!(
            snapshot.iter().any(|(key, _, _, value)| {
                let recorded = key
                    .key()
                    .labels()
                    .map(|label| (label.key(), label.value()))
                    .collect::<Vec<_>>();
                key.kind() == MetricKind::Counter
                    && key.key().name() == name
                    && recorded.len() == labels.len()
                    && labels.iter().all(|label| recorded.contains(label))
                    && matches!(value, DebugValue::Counter(1))
            }),
            "expected {name} with bounded labels {labels:?}: {snapshot:?}"
        );
    }

    /// Verify that successful shell resolution records only the fixed PowerShell labels.
    #[test]
    fn recipe_shell_resolution_records_bounded_success_telemetry() {
        let (result, snapshot, events) =
            record(|| instrument_recipe_shell_resolution(|| Ok(RecipeShell::PowerShell)));
        assert_eq!(
            result.expect("resolution should succeed"),
            RecipeShell::PowerShell
        );
        assert_counter(
            &snapshot,
            RECIPE_SHELL_RESOLUTIONS_TOTAL,
            &[
                ("recipe_shell", "powershell"),
                ("outcome", "success"),
                ("error_category", "none"),
            ],
        );
        assert!(events.iter().any(|event| {
            event.contains("recipe_shell=\"powershell\"")
                && event.contains("outcome=\"success\"")
                && event.contains("error_category=\"none\"")
        }));
    }

    /// Verify that Bash probe failure records only its fixed launch category.
    #[test]
    fn bash_preflight_records_bounded_failure_telemetry() {
        let (result, snapshot, events) = record(|| {
            instrument_bash_preflight(BashProbeOutcome::NotFound, || -> anyhow::Result<()> {
                bail!("do not expose this process detail")
            })
        });
        assert!(result.is_err(), "the injected preflight should fail");
        assert_counter(
            &snapshot,
            BASH_PREFLIGHT_TOTAL,
            &[
                ("recipe_shell", "bash"),
                ("outcome", "error"),
                ("probe_outcome", "not_found"),
            ],
        );
        assert!(events.iter().any(|event| {
            event.contains("recipe_shell=\"bash\"")
                && event.contains("outcome=\"error\"")
                && event.contains("probe_outcome=\"not_found\"")
                && !event.contains("do not expose this process detail")
        }));
    }
}
