//! Bounded structural telemetry for the rendered manifest.
//!
//! This module records only fixed-vocabulary aggregate counts derived from the
//! loaded manifest shape (variables, macros, rules, actions, targets, and
//! recipe variants). It never emits manifest text, paths, recipe contents,
//! variable values, or descriptions, because rendered manifest values can
//! carry secret material derived from `env()` interpolation.

use crate::ast::NetsukeManifest;
use metrics::{counter, describe_counter};
use std::sync::Once;
use tracing::trace_span;

/// Metric counting manifest structural summaries emitted by the runner.
const MANIFEST_STRUCTURES_TOTAL: &str = "netsuke_runner_manifest_structures_total";

/// Describe the manifest structure counter once per process.
fn describe_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            MANIFEST_STRUCTURES_TOTAL,
            "Counts runner-owned manifest structural summaries by collection sizes."
        );
    });
}

/// Emit a bounded structural summary of the rendered manifest.
///
/// The summary is emitted at `TRACE` level using fixed-vocabulary integer
/// fields only, so no manifest text can cross the telemetry boundary.
///
/// # Examples
///
/// ```
/// # use netsuke::ast::NetsukeManifest;
/// # use netsuke::runner::manifest_structure_telemetry::record_manifest_structure;
/// let manifest = NetsukeManifest::default();
/// record_manifest_structure(&manifest);
/// ```
///
/// # Panics
///
/// Panics if the global metrics recorder is not installed. Callers must
/// initialise the metrics pipeline before invoking this function.
pub fn record_manifest_structure(manifest: &NetsukeManifest) {
    describe_metrics();
    let span = trace_span!(
        "runner.manifest.structure",
        variable_count = manifest.vars.len(),
        macro_count = manifest.macros.len(),
        rule_count = manifest.rules.len(),
        action_count = manifest.actions.len(),
        target_count = manifest.targets.len(),
        default_count = manifest.defaults.len(),
    );
    let _guard = span.enter();
    counter!(MANIFEST_STRUCTURES_TOTAL).increment(1);
}
