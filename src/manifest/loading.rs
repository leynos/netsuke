//! Contains loading-orchestrator notifications outside pure manifest transforms.

use super::{ManifestLoadStage, expand::ExpansionReport};
use metrics::{counter, describe_counter};
use std::sync::Once;

/// Metric name counting filtered manifest targets.
const FILTERED_TARGETS_TOTAL: &str = "netsuke_manifest_filtered_targets_total";
/// Metric name counting filtered manifest actions.
const FILTERED_ACTIONS_TOTAL: &str = "netsuke_manifest_filtered_actions_total";
/// Metric name counting filtered entries omitted from the bounded report.
const OMITTED_FILTERED_ENTRIES_TOTAL: &str = "netsuke_manifest_omitted_filtered_entries_total";

/// Register unlabeled normal-manifest-loading metric descriptions once.
fn describe_expansion_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            FILTERED_TARGETS_TOTAL,
            "Counts filtered target entries from normal manifest loading; no labels."
        );
        describe_counter!(
            FILTERED_ACTIONS_TOTAL,
            "Counts filtered action entries from normal manifest loading; no labels."
        );
        describe_counter!(
            OMITTED_FILTERED_ENTRIES_TOTAL,
            "Counts filtered entries omitted from normal manifest-loading reports; no labels."
        );
    });
}

/// Emit tracing events for a manifest expansion report.
///
/// Expansion itself is a pure transformation that reports filtering through
/// [`ExpansionReport`]; this orchestrator-side helper owns the telemetry
/// policy. The report's fields are already bounded and non-sensitive (name
/// hash, expression length), so they can be logged verbatim.
pub(super) fn trace_expansion_report(report: &ExpansionReport) {
    describe_expansion_metrics();
    counter!(FILTERED_TARGETS_TOTAL)
        .increment(u64::try_from(report.stats.filtered_targets).unwrap_or(u64::MAX));
    counter!(FILTERED_ACTIONS_TOTAL)
        .increment(u64::try_from(report.stats.filtered_actions).unwrap_or(u64::MAX));
    counter!(OMITTED_FILTERED_ENTRIES_TOTAL)
        .increment(u64::try_from(report.omitted_filtered_entries).unwrap_or(u64::MAX));
    for entry in &report.filtered_entries {
        tracing::debug!(
            section = entry.section.as_str(),
            entry_name_hash = entry.entry_name_hash.as_str(),
            iteration_index = entry.iteration_index,
            when_expression_len = entry.when_expression_len,
            when_result = false,
            "filtered manifest entry by when expression"
        );
    }
    tracing::debug!(
        filtered_targets = report.stats.filtered_targets,
        filtered_actions = report.stats.filtered_actions,
        filtered_entry_count = report.stats.filtered_targets + report.stats.filtered_actions,
        omitted_filtered_entries = report.omitted_filtered_entries,
        "expanded manifest foreach and when directives"
    );
}

/// Invoke the stage callback when present.
pub(super) fn notify_stage(
    on_stage: &mut Option<&mut dyn FnMut(ManifestLoadStage)>,
    stage: ManifestLoadStage,
) {
    if let Some(callback) = on_stage.as_mut() {
        callback(stage);
    }
}
