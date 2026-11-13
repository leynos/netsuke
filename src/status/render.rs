//! String rendering for status lines, shared by both reporters.
//!
//! Every reporter emits localized text built the same way, so the formatters
//! live in one place: a change to a status line's wording or structure applies
//! to the `indicatif` and accessible paths at once. Splitting them out also
//! keeps each module within the repository's 400-line cap.

use crate::localization::{self, keys};
use crate::output_prefs::OutputPrefs;

use super::{LocalizationKey, StageNumber};

/// Render a stage label with current and total indices and a description.
pub(super) fn stage_label(current: StageNumber, total: StageNumber, description: &str) -> String {
    localization::message(keys::STATUS_STAGE_LABEL)
        .with_arg("current", current.get().to_string())
        .with_arg("total", total.get().to_string())
        .with_arg("description", description)
        .to_string()
}

/// Render a stage summary combining a state key's text with the stage label.
pub(super) fn stage_summary(
    state_key: LocalizationKey,
    current: StageNumber,
    total: StageNumber,
    description: &str,
) -> String {
    let state = localization::message(state_key.as_str()).to_string();
    let label = stage_label(current, total, description);
    localization::message(keys::STATUS_STAGE_SUMMARY)
        .with_arg("state", state)
        .with_arg("label", label)
        .to_string()
}

/// Render a task-progress line, appending the description when provided.
pub(super) fn task_progress_update(current: u32, total: u32, description: &str) -> String {
    let task = localization::message(keys::STATUS_TASK_PROGRESS_LABEL)
        .with_arg("current", current.to_string())
        .with_arg("total", total.to_string())
        .to_string();
    if description.is_empty() {
        return task;
    }
    localization::message(keys::STATUS_TASK_PROGRESS_UPDATE)
        .with_arg("task", task)
        .with_arg("description", description)
        .to_string()
}

/// Render the completion line with the tool-localized message and prefix.
pub(super) fn format_completion_line(prefs: OutputPrefs, tool_key: LocalizationKey) -> String {
    let tool = localization::message(tool_key.as_str());
    let prefix = prefs.success_prefix();
    let message = localization::message(keys::STATUS_COMPLETE).with_arg("tool", tool);
    format!("{prefix} {message}")
}
