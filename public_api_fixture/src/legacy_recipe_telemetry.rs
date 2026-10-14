//! Type-check the public legacy-recipe telemetry metric names.

use netsuke::runner::{LEGACY_RECIPE_EXECUTION_DURATION, LEGACY_RECIPE_EXECUTIONS_TOTAL};

/// The metric constants an external observability embedder imports.
pub const METRIC_NAMES: [&str; 2] = [
    LEGACY_RECIPE_EXECUTIONS_TOTAL,
    LEGACY_RECIPE_EXECUTION_DURATION,
];
