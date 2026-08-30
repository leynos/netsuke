//! Contains loading-orchestrator notifications outside pure manifest transforms.

use super::{ManifestLoadStage, expand::ExpansionReport};

/// Emit tracing events for a manifest expansion report.
///
/// Expansion itself is a pure transformation that reports filtering through
/// [`ExpansionReport`]; this orchestrator-side helper owns the telemetry
/// policy. The report's fields are already bounded and non-sensitive (name
/// hash, expression length), so they can be logged verbatim.
pub(super) fn trace_expansion_report(report: &ExpansionReport) {
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
