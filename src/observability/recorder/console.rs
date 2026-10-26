//! Admit only the fixed console-scheduling generation and build metric series.

use super::labels::exact_labels;
use metrics::Key;
use metrics_util::MetricKind;
use netsuke::runner::{
    BUNDLE_CONSOLE_EDGES_TOTAL, BUNDLE_GENERATION_DURATION, BUNDLE_GENERATIONS_TOTAL,
    NINJA_BUILD_CONSOLE_EDGES_TOTAL, NINJA_BUILD_EXECUTION_DURATION, NINJA_BUILD_EXECUTIONS_TOTAL,
};

/// Bound scheduling labels to the presence of selected console edges.
const SCHEDULING: &[&str] = &["enabled", "disabled"];
/// Bound operation outcomes to completed successes and errors.
const OUTCOMES: &[&str] = &["success", "error"];
/// Bound Ninja invocation errors without retaining process details.
const BUILD_ERRORS: &[&str] = &["none", "io"];
/// Bound bundle errors to the generator's fixed classification vocabulary.
const GENERATION_ERRORS: &[&str] = &[
    "none",
    "missing_action",
    "console_dependency_only",
    "console_pool_conflict",
    "command_list",
    "unsafe_ninja_value",
    "power_shell_recipe_too_large",
    "format",
    "dyndep_files_required",
    "reserved_output_path",
    "unsupported_path_character",
    "unsafe_ninja_path",
];

/// Report whether a description belongs to the console-scheduling metric set.
pub(super) fn is_console_metric(name: &str) -> bool {
    matches!(
        name,
        BUNDLE_CONSOLE_EDGES_TOTAL
            | BUNDLE_GENERATION_DURATION
            | BUNDLE_GENERATIONS_TOTAL
            | NINJA_BUILD_CONSOLE_EDGES_TOTAL
            | NINJA_BUILD_EXECUTION_DURATION
            | NINJA_BUILD_EXECUTIONS_TOTAL
    )
}

/// Admit only the metric kind and exact finite labels emitted by the runner.
pub(super) fn accepts_registration(key: &Key, kind: MetricKind) -> bool {
    let errors = match (key.name(), kind) {
        (BUNDLE_CONSOLE_EDGES_TOTAL | NINJA_BUILD_CONSOLE_EDGES_TOTAL, MetricKind::Counter) => {
            return exact_labels(key, &[("outcome", OUTCOMES)]);
        }
        (BUNDLE_GENERATIONS_TOTAL, MetricKind::Counter)
        | (BUNDLE_GENERATION_DURATION, MetricKind::Histogram) => GENERATION_ERRORS,
        (NINJA_BUILD_EXECUTIONS_TOTAL, MetricKind::Counter)
        | (NINJA_BUILD_EXECUTION_DURATION, MetricKind::Histogram) => BUILD_ERRORS,
        _ => return false,
    };
    exact_labels(
        key,
        &[
            ("outcome", OUTCOMES),
            ("error_category", errors),
            ("console_scheduling", SCHEDULING),
        ],
    )
}
