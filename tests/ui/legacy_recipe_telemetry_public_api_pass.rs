//! Compile-pass fixture for public legacy-recipe telemetry metric names.

use netsuke::runner::{LEGACY_RECIPE_EXECUTION_DURATION, LEGACY_RECIPE_EXECUTIONS_TOTAL};

/// Name the metric constants an external observability embedder imports.
const METRIC_NAMES: [&str; 2] = [
    LEGACY_RECIPE_EXECUTIONS_TOTAL,
    LEGACY_RECIPE_EXECUTION_DURATION,
];

fn main() {
    assert_eq!(
        METRIC_NAMES,
        [
            "netsuke_runner_legacy_recipe_executions_total",
            "netsuke_runner_legacy_recipe_execution_duration_seconds",
        ]
    );
}
