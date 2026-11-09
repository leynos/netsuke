//! `indicatif`-backed stage progress reporting.
//!
//! Holds [`super`]'s [`IndicatifReporter`]. The localized text it writes is
//! built by the formatters in [`super::render`], which the accessible reporter
//! shares, so a status line's wording is defined once for both paths.

use std::io::{self, Write};
use std::sync::Mutex;

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

use crate::localization::{self, keys};
use crate::output_prefs::OutputPrefs;

use super::pipeline::PIPELINE_STAGE_TOTAL;
use super::render::{format_completion_line, stage_label, stage_summary, task_progress_update};
use super::{LocalizationKey, PipelineStage, StageNumber, StatusReporter};

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

/// One stage line's next state, as a single unit.
///
/// The three fields describe a single transition and are never varied
/// independently, so grouping them keeps the call sites readable and stops
/// `set_stage_state` carrying a loose bool whose meaning is positional.
#[derive(Debug, Clone, Copy)]
struct StageTransition {
    /// Zero-based index of the stage bar to update.
    index: usize,
    /// Localization key naming the state the stage moves to.
    status_key: LocalizationKey,
    /// Whether the line is finished rather than merely repainted.
    finish_line: bool,
}

/// Standard reporter backed by `indicatif::MultiProgress`.
///
/// The plain-text sink defaults to [`io::Stderr`]. Tests supply an in-memory
/// writer through the test-only `with_writer` constructor so the text path can
/// be captured byte for byte, including any prefix an edit adds *at the write
/// site* — which a formatter-level assertion cannot see.
pub struct IndicatifReporter<W: Write + Send = io::Stderr> {
    /// Output preferences controlling prefixes and indentation.
    prefs: OutputPrefs,
    /// The mutable progress state, locked per update. Crate-visible so
    /// `status/tests.rs` can white-box the rendered messages.
    pub(crate) state: Mutex<IndicatifState>,
    /// The plain-text line sink, locked per write. Crate-visible so
    /// `status/tests.rs` can read back the bytes the reporter wrote.
    pub(crate) writer: Mutex<W>,
}

impl IndicatifReporter {
    /// Build a multi-progress reporter with one line per pipeline stage.
    #[must_use]
    pub fn new(prefs: OutputPrefs, force_text_task_updates: bool) -> Self {
        Self::with_draw_target(
            prefs,
            force_text_task_updates,
            ProgressDrawTarget::stderr_with_hz(12),
        )
    }

    /// Build a reporter whose draw target follows the resolved display plan.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let reporter = IndicatifReporter::with_display_plan(prefs, true, false);
    /// ```
    #[must_use]
    pub(crate) fn with_display_plan(
        prefs: OutputPrefs,
        force_text_task_updates: bool,
        live_progress: bool,
    ) -> Self {
        let draw_target = if live_progress {
            ProgressDrawTarget::stderr_with_hz(12)
        } else {
            ProgressDrawTarget::hidden()
        };
        Self::with_draw_target(prefs, force_text_task_updates, draw_target)
    }

    /// Build a reporter around an explicit progress draw target.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let reporter = IndicatifReporter::with_draw_target(prefs, false, target);
    /// ```
    #[must_use]
    fn with_draw_target(
        prefs: OutputPrefs,
        force_text_task_updates: bool,
        draw_target: ProgressDrawTarget,
    ) -> Self {
        Self::assemble(prefs, force_text_task_updates, draw_target, io::stderr())
    }

    /// Build a reporter with explicit task-update text fallback control.
    #[must_use]
    pub fn with_force_text_task_updates(prefs: OutputPrefs, force_text_task_updates: bool) -> Self {
        Self::new(prefs, force_text_task_updates)
    }
}

impl<W: Write + Send> IndicatifReporter<W> {
    /// Build a reporter that writes its plain-text lines to `writer`.
    ///
    /// This is the capture seam for the text path. Asserting the value
    /// returned by [`task_progress_update`] cannot prove the *write* is clean:
    /// an edit could add a prefix at the call site, leaving the formatter's
    /// result untouched. Routing the write through a supplied sink makes the
    /// bytes the reporter actually emits observable.
    ///
    /// The draw target is hidden, so every plain line the reporter would
    /// mirror to stderr lands in `writer` instead. Capture therefore sees the
    /// complete output rather than the subset a live target would draw.
    #[cfg(test)]
    pub(crate) fn with_writer(
        prefs: OutputPrefs,
        force_text_task_updates: bool,
        writer: W,
    ) -> Self {
        Self::assemble(
            prefs,
            force_text_task_updates,
            ProgressDrawTarget::hidden(),
            writer,
        )
    }

    /// Build a reporter from its parts, sharing one body across constructors.
    #[must_use]
    fn assemble(
        prefs: OutputPrefs,
        force_text_task_updates: bool,
        draw_target: ProgressDrawTarget,
        writer: W,
    ) -> Self {
        let progress = MultiProgress::with_draw_target(draw_target);
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
            writer: Mutex::new(writer),
        }
    }

    /// Return whether the Stage-6 bar is the active progress surface.
    fn is_stage6_active(state: &IndicatifState) -> bool {
        state.running_index == Some(STAGE6_INDEX) && STAGE6_INDEX < state.bars.len()
    }

    /// Write one plain-text line, terminator included, to the sink.
    ///
    /// The text path's single write site, so a capture over `writer` sees
    /// exactly the bytes emitted: the message is the argument, and nothing is
    /// prepended or appended here beyond `writeln!`'s newline.
    fn write_plain_line(&self, message: &str) {
        let mut writer = self
            .writer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(writeln!(writer, "{message}"));
    }

    /// Refresh a stage bar's message, mirroring the update to the plain sink
    /// when the draw target is hidden.
    ///
    /// Takes the already-locked state so callers can hold one lock across a
    /// stage transition; only the writer is newly locked here. The stage's
    /// index, state key and whether it closes the line arrive together as one
    /// [`StageTransition`], because they describe a single transition and are
    /// never varied independently.
    fn set_stage_state(&self, state: &mut IndicatifState, transition: StageTransition) {
        let StageTransition {
            index,
            status_key,
            finish_line,
        } = transition;
        let Some(current_raw) = u32::try_from(index + 1).ok() else {
            return;
        };
        let current = StageNumber::new_unchecked(current_raw);
        let description = state.descriptions.get(index).map_or("", String::as_str);
        let message = stage_summary(status_key, current, PIPELINE_STAGE_TOTAL, description);
        if state.is_hidden {
            self.write_plain_line(&message);
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

impl<W: Write + Send> Drop for IndicatifReporter<W> {
    fn drop(&mut self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.completed {
            return;
        }
        if let Some(index) = state.running_index.take() {
            self.set_stage_state(
                &mut state,
                StageTransition {
                    index,
                    status_key: LocalizationKey::new(keys::STATUS_STATE_FAILED),
                    finish_line: true,
                },
            );
        }
        // Keep `state` alive so the MultiProgress flush completes before drop.
        let _ = &state.progress;
    }
}

impl<W: Write + Send> StatusReporter for IndicatifReporter<W> {
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
            self.set_stage_state(
                &mut state,
                StageTransition {
                    index: previous,
                    status_key: LocalizationKey::new(keys::STATUS_STATE_DONE),
                    finish_line: true,
                },
            );
        }

        self.set_stage_state(
            &mut state,
            StageTransition {
                index,
                status_key: LocalizationKey::new(keys::STATUS_STATE_RUNNING),
                finish_line: false,
            },
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
            self.write_plain_line(&task);
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
            self.set_stage_state(
                &mut state,
                StageTransition {
                    index,
                    status_key: LocalizationKey::new(keys::STATUS_STATE_DONE),
                    finish_line: true,
                },
            );
        }
        state.completed = true;
        // Keep `state` alive so the MultiProgress flush completes before drop.
        let _ = &state.progress;

        let line = format_completion_line(self.prefs, tool_key);
        self.write_plain_line(&line);
    }
}
