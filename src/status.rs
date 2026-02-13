//! Pipeline status reporting for accessible and standard output modes.

use crate::localization::{self, keys};
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::io::{self, Write};
use std::sync::Mutex;
use thiserror::Error;

/// Total count of user-visible pipeline stages.
pub const PIPELINE_STAGE_COUNT: u32 = 6;

/// Validation error when constructing a [`StageNumber`].
#[derive(Debug, Error, Copy, Clone, PartialEq, Eq)]
pub enum StageNumberError {
    /// Provided value is not within the inclusive stage range.
    #[error("stage number {0} is out of range (expected 1..={PIPELINE_STAGE_COUNT})")]
    OutOfRange(u32),
}

/// Validated stage index in the range `1..=PIPELINE_STAGE_COUNT`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StageNumber(u32);

impl StageNumber {
    /// Build a validated stage number.
    ///
    /// # Errors
    ///
    /// Returns [`StageNumberError::OutOfRange`] when `value` is not between 1
    /// and [`PIPELINE_STAGE_COUNT`] inclusive.
    #[must_use = "validate and use the constructed stage number"]
    pub const fn new(value: u32) -> Result<Self, StageNumberError> {
        if value >= 1 && value <= PIPELINE_STAGE_COUNT {
            Ok(Self(value))
        } else {
            Err(StageNumberError::OutOfRange(value))
        }
    }

    /// Return the raw numeric stage index.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Localized description text for a pipeline stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageDescription(String);

impl StageDescription {
    /// Wrap a localized stage description.
    #[must_use]
    pub const fn new(value: String) -> Self {
        Self(value)
    }

    /// Borrow the wrapped description.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for StageDescription {
    fn from(value: String) -> Self {
        Self::new(value)
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

const PIPELINE_STAGE_TOTAL: StageNumber = StageNumber(PIPELINE_STAGE_COUNT);
#[path = "status_pipeline.rs"]
mod pipeline;
pub use pipeline::{PipelineStage, report_pipeline_stage};

fn stage_label(current: StageNumber, total: StageNumber, description: &StageDescription) -> String {
    localization::message(keys::STATUS_STAGE_LABEL)
        .with_arg("current", current.get().to_string())
        .with_arg("total", total.get().to_string())
        .with_arg("description", description.as_str())
        .to_string()
}

fn stage_summary(
    state_key: LocalizationKey,
    current: StageNumber,
    total: StageNumber,
    description: &StageDescription,
) -> String {
    let state = localization::message(state_key.as_str()).to_string();
    let label = stage_label(current, total, description);
    localization::message(keys::STATUS_STAGE_SUMMARY)
        .with_arg("state", state)
        .with_arg("label", label)
        .to_string()
}

/// Reports pipeline stage transitions and completion.
pub trait StatusReporter {
    /// Emit a stage update.
    fn report_stage(&self, current: StageNumber, total: StageNumber, description: StageDescription);
    /// Emit a final completion message.
    fn report_complete(&self, tool_key: LocalizationKey);
}

/// Accessible reporter that emits static lines.
pub struct AccessibleReporter;

impl StatusReporter for AccessibleReporter {
    fn report_stage(
        &self,
        current: StageNumber,
        total: StageNumber,
        description: StageDescription,
    ) {
        let message = stage_label(current, total, &description);
        drop(writeln!(io::stderr(), "{message}"));
    }

    fn report_complete(&self, tool_key: LocalizationKey) {
        let tool = localization::message(tool_key.as_str());
        let message = localization::message(keys::STATUS_COMPLETE).with_arg("tool", tool);
        drop(writeln!(io::stderr(), "{message}"));
    }
}

/// Reporter that suppresses status output.
pub struct SilentReporter;

impl StatusReporter for SilentReporter {
    fn report_stage(
        &self,
        _current: StageNumber,
        _total: StageNumber,
        _description: StageDescription,
    ) {
    }
    fn report_complete(&self, _tool_key: LocalizationKey) {}
}

#[derive(Debug)]
struct IndicatifState {
    progress: MultiProgress,
    bars: Vec<ProgressBar>,
    descriptions: Vec<StageDescription>,
    running_index: Option<usize>,
    completed: bool,
    is_hidden: bool,
}

/// Standard reporter backed by `indicatif::MultiProgress`.
pub struct IndicatifReporter {
    state: Mutex<IndicatifState>,
}

impl IndicatifReporter {
    /// Build a multi-progress reporter with one line per pipeline stage.
    #[must_use]
    pub fn new() -> Self {
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
            }),
        }
    }

    fn set_stage_state(
        state: &mut IndicatifState,
        index: usize,
        status_key: LocalizationKey,
        finish_line: bool,
    ) {
        let Ok(current_raw) = u32::try_from(index + 1) else {
            return;
        };
        let Ok(current) = StageNumber::new(current_raw) else {
            return;
        };
        let description = state
            .descriptions
            .get(index)
            .cloned()
            .unwrap_or_else(|| StageDescription::new(String::new()));
        let message = stage_summary(status_key, current, PIPELINE_STAGE_TOTAL, &description);
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
        Self::new()
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
        let _ = &state.progress;
    }
}

impl StatusReporter for IndicatifReporter {
    fn report_stage(
        &self,
        current: StageNumber,
        _total: StageNumber,
        description: StageDescription,
    ) {
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

        let Some(existing_description) = state.descriptions.get_mut(index) else {
            return;
        };
        *existing_description = description;
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
        let _ = &state.progress;

        let tool = localization::message(tool_key.as_str());
        let message = localization::message(keys::STATUS_COMPLETE).with_arg("tool", tool);
        drop(writeln!(io::stderr(), "{message}"));
    }
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
