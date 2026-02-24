//! Pipeline status reporting for accessible and standard output modes.

use crate::localization::{self, keys};
use crate::output_prefs::OutputPrefs;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::io::{self, Write};
use std::sync::Mutex;

/// Thin wrapper for a 1-based stage index.
///
/// All current call sites derive stage numbers from the `PipelineStage` enum
/// whose discriminants are statically known to be in range, so validation at
/// construction time is unnecessary.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StageNumber(u32);

impl StageNumber {
    /// Build a stage number without runtime validation.
    #[must_use]
    pub const fn new_unchecked(value: u32) -> Self {
        Self(value)
    }

    /// Return the raw numeric stage index.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Fluent localization key used for status output messages.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct LocalizationKey(&'static str);

impl LocalizationKey {
    /// Wrap a static Fluent key string.
    #[must_use]
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    /// Return the wrapped Fluent key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl From<&'static str> for LocalizationKey {
    fn from(s: &'static str) -> Self {
        Self::new(s)
    }
}

#[path = "status_pipeline.rs"]
mod pipeline;
use pipeline::PIPELINE_STAGE_TOTAL;
pub use pipeline::{PipelineStage, report_pipeline_stage};

fn stage_label(current: StageNumber, total: StageNumber, description: &str) -> String {
    localization::message(keys::STATUS_STAGE_LABEL)
        .with_arg("current", current.get().to_string())
        .with_arg("total", total.get().to_string())
        .with_arg("description", description)
        .to_string()
}

fn stage_summary(
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

fn task_progress_label(current: u32, total: u32) -> String {
    localization::message(keys::STATUS_TASK_PROGRESS_LABEL)
        .with_arg("current", current.to_string())
        .with_arg("total", total.to_string())
        .to_string()
}

fn task_progress_update(current: u32, total: u32, description: &str) -> String {
    let task = task_progress_label(current, total);
    if description.is_empty() {
        return task;
    }
    localization::message(keys::STATUS_TASK_PROGRESS_UPDATE)
        .with_arg("task", task)
        .with_arg("description", description)
        .to_string()
}

/// Reports pipeline stage transitions and completion.
pub trait StatusReporter {
    /// Emit a stage update.
    fn report_stage(&self, current: StageNumber, total: StageNumber, description: &str);
    /// Emit task progress for Stage 6 execution.
    fn report_task_progress(&self, _current: u32, _total: u32, _description: &str) {}
    /// Emit a final completion message.
    fn report_complete(&self, tool_key: LocalizationKey);
}

/// Accessible reporter that emits static, labelled lines to stderr.
///
/// Each line follows the pattern `Stage N/M: Description`, using
/// localized messages from the Fluent resource bundle. Completion
/// messages are prefixed with a semantic `Success:` label (with or
/// without an emoji glyph depending on [`OutputPrefs`]).
pub struct AccessibleReporter {
    prefs: OutputPrefs,
}

impl AccessibleReporter {
    /// Create a reporter with the given output preferences.
    #[must_use]
    pub const fn new(prefs: OutputPrefs) -> Self {
        Self { prefs }
    }
}

impl StatusReporter for AccessibleReporter {
    fn report_stage(&self, current: StageNumber, total: StageNumber, description: &str) {
        let message = stage_label(current, total, description);
        drop(writeln!(io::stderr(), "{message}"));
    }

    fn report_complete(&self, tool_key: LocalizationKey) {
        let tool = localization::message(tool_key.as_str());
        let prefix = self.prefs.success_prefix();
        let message = localization::message(keys::STATUS_COMPLETE).with_arg("tool", tool);
        drop(writeln!(io::stderr(), "{prefix} {message}"));
    }

    fn report_task_progress(&self, current: u32, total: u32, description: &str) {
        let message = task_progress_update(current, total, description);
        drop(writeln!(io::stderr(), "{message}"));
    }
}

/// Reporter that suppresses status output.
pub struct SilentReporter;

impl StatusReporter for SilentReporter {
    fn report_stage(&self, _current: StageNumber, _total: StageNumber, _description: &str) {}
    fn report_complete(&self, _tool_key: LocalizationKey) {}
}

#[derive(Debug)]
struct IndicatifState {
    progress: MultiProgress,
    bars: Vec<ProgressBar>,
    descriptions: Vec<String>,
    running_index: Option<usize>,
    completed: bool,
    is_hidden: bool,
    force_text_task_updates: bool,
    last_task_progress: Option<(u32, u32)>,
}

/// Standard reporter backed by `indicatif::MultiProgress`.
pub struct IndicatifReporter {
    state: Mutex<IndicatifState>,
}

impl IndicatifReporter {
    /// Build a multi-progress reporter with one line per pipeline stage.
    #[must_use]
    pub fn new(force_text_task_updates: bool) -> Self {
        let progress = MultiProgress::with_draw_target(ProgressDrawTarget::stderr_with_hz(12));
        progress.set_move_cursor(false);
        let style = ProgressStyle::with_template("{msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner());

        let mut bars = Vec::with_capacity(PipelineStage::ALL.len());
        let mut descriptions = Vec::with_capacity(PipelineStage::ALL.len());
        for stage in PipelineStage::ALL {
            let description = stage.description(None);
            let current = stage.index();
            let bar = progress.add(ProgressBar::new(1));
            bar.set_style(style.clone());
            bar.set_message(stage_summary(
                LocalizationKey::new(keys::STATUS_STATE_PENDING),
                current,
                PIPELINE_STAGE_TOTAL,
                &description,
            ));
            bars.push(bar);
            descriptions.push(description);
        }

        Self {
            state: Mutex::new(IndicatifState {
                is_hidden: progress.is_hidden(),
                progress,
                bars,
                descriptions,
                running_index: None,
                completed: false,
                force_text_task_updates,
                last_task_progress: None,
            }),
        }
    }

    fn set_stage_state(
        state: &mut IndicatifState,
        index: usize,
        status_key: LocalizationKey,
        finish_line: bool,
    ) {
        let Some(current_raw) = u32::try_from(index + 1).ok() else {
            return;
        };
        let current = StageNumber::new_unchecked(current_raw);
        let description = state.descriptions.get(index).map_or("", String::as_str);
        let message = stage_summary(status_key, current, PIPELINE_STAGE_TOTAL, description);
        if state.is_hidden {
            drop(writeln!(io::stderr(), "{message}"));
            return;
        }
        if let Some(bar) = state.bars.get(index) {
            if finish_line {
                bar.finish_with_message(message);
            } else {
                bar.set_message(message);
            }
        }
    }
}

impl Default for IndicatifReporter {
    fn default() -> Self {
        Self::new(false)
    }
}

impl Drop for IndicatifReporter {
    fn drop(&mut self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.completed {
            return;
        }
        if let Some(index) = state.running_index.take() {
            Self::set_stage_state(
                &mut state,
                index,
                LocalizationKey::new(keys::STATUS_STATE_FAILED),
                true,
            );
        }
        // Keep `state` alive so the MultiProgress flush completes before drop.
        let _ = &state.progress;
    }
}

impl StatusReporter for IndicatifReporter {
    fn report_stage(&self, current: StageNumber, _total: StageNumber, description: &str) {
        let Ok(index) = usize::try_from(current.get().saturating_sub(1)) else {
            return;
        };

        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if index >= state.bars.len() {
            return;
        }

        if let Some(existing) = state.descriptions.get_mut(index) {
            description.clone_into(existing);
        }
        if let Some(previous) = state.running_index
            && previous != index
        {
            Self::set_stage_state(
                &mut state,
                previous,
                LocalizationKey::new(keys::STATUS_STATE_DONE),
                true,
            );
        }

        Self::set_stage_state(
            &mut state,
            index,
            LocalizationKey::new(keys::STATUS_STATE_RUNNING),
            false,
        );
        if index != stage6_index() {
            state.last_task_progress = None;
        }
        state.running_index = Some(index);
    }

    fn report_task_progress(&self, current: u32, total: u32, description: &str) {
        if total == 0 || current == 0 || current > total {
            return;
        }
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let stage_index = stage6_index();
        if stage_index >= state.bars.len() || state.running_index != Some(stage_index) {
            return;
        }
        if let Some((last_current, last_total)) = state.last_task_progress
            && (total != last_total || current < last_current)
        {
            return;
        }
        state.last_task_progress = Some((current, total));
        let task = task_progress_update(current, total, description);
        if state.is_hidden || state.force_text_task_updates {
            drop(writeln!(io::stderr(), "{task}"));
            return;
        }
        let Some(stage_current_raw) = u32::try_from(stage_index + 1).ok() else {
            return;
        };
        let stage_current = StageNumber::new_unchecked(stage_current_raw);
        let stage_description = state
            .descriptions
            .get(stage_index)
            .map_or("", String::as_str);
        let state_label = localization::message(keys::STATUS_STATE_RUNNING).to_string();
        let stage_line = stage_label(stage_current, PIPELINE_STAGE_TOTAL, stage_description);
        let message = localization::message(keys::STATUS_STAGE_SUMMARY_WITH_TASK)
            .with_arg("state", state_label)
            .with_arg("label", stage_line)
            .with_arg("task_progress", &task)
            .to_string();
        if let Some(bar) = state.bars.get(stage_index) {
            bar.set_message(message);
        }
    }

    fn report_complete(&self, tool_key: LocalizationKey) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(index) = state.running_index.take() {
            Self::set_stage_state(
                &mut state,
                index,
                LocalizationKey::new(keys::STATUS_STATE_DONE),
                true,
            );
        }
        state.completed = true;
        // Keep `state` alive so the MultiProgress flush completes before drop.
        let _ = &state.progress;

        let tool = localization::message(tool_key.as_str());
        let message = localization::message(keys::STATUS_COMPLETE).with_arg("tool", tool);
        drop(writeln!(io::stderr(), "{message}"));
    }
}

const fn stage6_index() -> usize {
    5
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
