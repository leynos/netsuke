//! Tests bounded telemetry for complete generated-recipe runner operations.

use super::super::{
    LEGACY_RECIPE_EXECUTION_DURATION, LEGACY_RECIPE_EXECUTIONS_TOTAL, LegacyRecipeOperation,
    instrument_legacy_recipe_operation,
};
use super::{Snapshot, record};
use crate::recipe_shell::RecipeShell;
use anyhow::bail;
use metrics_util::{MetricKind, debugging::DebugValue};

/// Assert that `snapshot` records one histogram sample with exactly `labels`.
fn assert_histogram(snapshot: &Snapshot, name: &str, labels: &[(&str, &str)]) {
    let recorded = snapshot
        .iter()
        .filter(|(key, _, _, _)| key.kind() == MetricKind::Histogram && key.key().name() == name)
        .collect::<Vec<_>>();
    assert_eq!(
        recorded.len(),
        1,
        "expected exactly one {name} histogram: {snapshot:?}"
    );
    assert!(
        recorded.first().is_some_and(|(key, _, _, value)| {
            let actual_labels = key
                .key()
                .labels()
                .map(|label| (label.key(), label.value()))
                .collect::<Vec<_>>();
            actual_labels.len() == labels.len()
                && labels.iter().all(|label| actual_labels.contains(label))
                && matches!(value, DebugValue::Histogram(samples) if samples.len() == 1)
        }),
        "expected one {name} sample with bounded labels {labels:?}: {snapshot:?}"
    );
}

/// Assert one counter increment and duration sample for a runner operation.
fn assert_legacy_recipe_operation(snapshot: &Snapshot, labels: [(&str, &str); 4]) {
    let counter_count = snapshot
        .iter()
        .filter(|(key, _, _, _)| {
            key.kind() == MetricKind::Counter && key.key().name() == LEGACY_RECIPE_EXECUTIONS_TOTAL
        })
        .count();
    assert_eq!(
        counter_count, 1,
        "expected exactly one {LEGACY_RECIPE_EXECUTIONS_TOTAL} counter: {snapshot:?}"
    );
    super::assert_counter(snapshot, LEGACY_RECIPE_EXECUTIONS_TOTAL, &labels);
    assert_histogram(snapshot, LEGACY_RECIPE_EXECUTION_DURATION, &labels);
}

/// Verify that shell labels cannot contain values outside the declared vocabulary.
#[test]
fn legacy_recipe_operation_uses_fixed_shell_labels() {
    assert_eq!(super::super::shell_label(RecipeShell::Posix), "posix");
    assert_eq!(
        super::super::shell_label(RecipeShell::PowerShell),
        "powershell"
    );
    assert_eq!(super::super::shell_label(RecipeShell::Bash), "bash");
}

/// Verify that a successful build operation emits exactly one bounded metric pair.
#[test]
fn legacy_recipe_operation_records_successful_build_telemetry() {
    let (result, snapshot, events) = record(|| {
        instrument_legacy_recipe_operation(
            LegacyRecipeOperation::Build,
            RecipeShell::PowerShell,
            || Ok(()),
        )
    });
    result.expect("the injected build operation should succeed");
    assert_legacy_recipe_operation(
        &snapshot,
        [
            ("operation", "build"),
            ("recipe_shell", "powershell"),
            ("outcome", "success"),
            ("failure_category", "none"),
        ],
    );
    assert!(events.iter().any(|event| {
        event.contains("operation=\"build\"")
            && event.contains("recipe_shell=\"powershell\"")
            && event.contains("outcome=\"success\"")
            && event.contains("failure_category=\"none\"")
    }));
}

/// Verify that failure telemetry excludes manifest and process-controlled details.
#[test]
fn legacy_recipe_operation_records_bounded_ninja_tool_failure_telemetry() {
    let sensitive_detail = "target='/tmp/unsafe path' status=137 tool=compdb";
    let (result, snapshot, events) = record(|| {
        instrument_legacy_recipe_operation(
            LegacyRecipeOperation::NinjaTool,
            RecipeShell::Bash,
            || -> anyhow::Result<()> { bail!("{sensitive_detail}") },
        )
    });
    assert!(
        result.is_err(),
        "the injected Ninja-tool operation should fail"
    );
    assert_legacy_recipe_operation(
        &snapshot,
        [
            ("operation", "ninja_tool"),
            ("recipe_shell", "bash"),
            ("outcome", "error"),
            ("failure_category", "other"),
        ],
    );
    assert!(events.iter().any(|event| {
        event.contains("operation=\"ninja_tool\"")
            && event.contains("recipe_shell=\"bash\"")
            && event.contains("outcome=\"error\"")
            && event.contains("failure_category=\"other\"")
            && !event.contains(sensitive_detail)
    }));
}
