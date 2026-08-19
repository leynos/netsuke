//! `indicatif`-backed stage progress reporting.
//!
//! Holds [`super`]'s [`IndicatifReporter`] and the string-rendering helpers
//! its write paths share with the accessible reporter, so `status.rs` stays
//! within the repository's 400-line cap.

use std::io::{self, Write};
use std::sync::Mutex;

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

use crate::localization::{self, keys};
use crate::output_prefs::OutputPrefs;

use super::pipeline::PIPELINE_STAGE_TOTAL;
use super::{LocalizationKey, PipelineStage, StageNumber, StatusReporter};

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

/// Render the completion line with the tool-localised message and prefix.
pub(super) fn format_completion_line(prefs: OutputPrefs, tool_key: LocalizationKey) -> String {
    let tool = localization::message(tool_key.as_str());
    let prefix = prefs.success_prefix();
    let message = localization::message(keys::STATUS_COMPLETE).with_arg("tool", tool);
    format!("{prefix} {message}")
}

/// The mutable progress state behind [`IndicatifReporter`].
#[derive(Debug)]
pub(crate) struct IndicatifState {
    /// The multi-line progress surface drawn to stderr.
    progress: MultiProgress,
    /// One bar per pipeline stage, in stage order.
    pub(crate) bars: Vec<ProgressBar>,
    /// The untranslated description text cached per stage.
    descriptions: Vec<String>,
    /// The stage index currently shown as running, when any.
    running_index: Option<usize>,
    /// Whether the reporter already emitted its completion message.
    completed: bool,
    /// Whether the underlying draw target renders nothing.
    pub(crate) is_hidden: bool,
    /// Whether task updates always fall back to plain text lines.
    force_text_task_updates: bool,
}

/// The zero-based index of the Stage-6 task-progress bar.
pub(super) const STAGE6_INDEX: usize = (PipelineStage::NinjaSynthesisAndExecution as usize) - 1;

/// Standard reporter backed by `indicatif::MultiProgress`.
pub struct IndicatifReporter {
    /// Output preferences controlling prefixes and indentation.
    prefs: OutputPrefs,
    /// The mutable progress state, locked per update. Crate-visible so
    /// `status_tests.rs` can white-box the rendered messages.
    pub(crate) state: Mutex<IndicatifState>,
}

impl IndicatifReporter {
    /// Build a multi-progress reporter with one line per pipeline stage.
    #[must_use]
    pub fn new(prefs: OutputPrefs, force_text_task_updates: bool) -> Self {
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
            prefs,
            state: Mutex::new(IndicatifState {
                is_hidden: progress.is_hidden(),
                progress,
                bars,
                descriptions,
                running_index: None,
                completed: false,
                force_text_task_updates,
            }),
        }
    }

    /// Build a reporter with explicit task-update text fallback control.
    #[must_use]
    pub fn with_force_text_task_updates(prefs: OutputPrefs, force_text_task_updates: bool) -> Self {
        Self::new(prefs, force_text_task_updates)
    }

    /// Return whether the Stage-6 bar is the active progress surface.
    fn is_stage6_active(state: &IndicatifState) -> bool {
        state.running_index == Some(STAGE6_INDEX) && STAGE6_INDEX < state.bars.len()
    }

    /// Refresh a stage bar's message, mirroring the update to plain stderr
    /// when the draw target is hidden.
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
        Self::with_force_text_task_updates(crate::output_prefs::resolve(None), false)
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
        state.running_index = Some(index);
    }

    fn report_task_progress(&self, current: u32, total: u32, description: &str) {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !Self::is_stage6_active(&state) {
            return;
        }
        let stage_index = STAGE6_INDEX;
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

        let line = format_completion_line(self.prefs, tool_key);
        drop(writeln!(io::stderr(), "{line}"));
    }
}
