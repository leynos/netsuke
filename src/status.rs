//! Pipeline status reporting for accessible and standard output modes.
//!
//! This module provides a [`StatusReporter`] trait plus concrete reporters for
//! both accessibility-first textual output and standard terminal progress
//! output. Standard mode uses `indicatif::MultiProgress` to keep stage summaries
//! persistent while the pipeline advances.

use crate::localization::{self, keys};
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::io::{self, Write};
use std::sync::Mutex;

fn stage_label(current: u32, total: u32, description: &str) -> String {
    localization::message(keys::STATUS_STAGE_LABEL)
        .with_arg("current", current.to_string())
        .with_arg("total", total.to_string())
        .with_arg("description", description)
        .to_string()
}

fn stage_summary(state_key: &'static str, current: u32, total: u32, description: &str) -> String {
    let state = localization::message(state_key).to_string();
    let label = stage_label(current, total, description);
    localization::message(keys::STATUS_STAGE_SUMMARY)
        .with_arg("state", state)
        .with_arg("label", label)
        .to_string()
}

/// Report pipeline progress to the user.
pub trait StatusReporter {
    /// Emit a status update for the given pipeline stage.
    fn report_stage(&self, current: u32, total: u32, description: &str);

    /// Emit a completion message after a successful pipeline run.
    fn report_complete(&self, tool_key: &'static str);
}

/// Accessible reporter: writes static, labelled lines to stderr.
pub struct AccessibleReporter;

impl StatusReporter for AccessibleReporter {
    fn report_stage(&self, current: u32, total: u32, description: &str) {
        let message = stage_label(current, total, description);
        // Intentionally discard the write result: status output failures should
        // not abort the build pipeline.
        drop(writeln!(io::stderr(), "{message}"));
    }

    fn report_complete(&self, tool_key: &'static str) {
        let tool = localization::message(tool_key);
        let message = localization::message(keys::STATUS_COMPLETE).with_arg("tool", tool);
        drop(writeln!(io::stderr(), "{message}"));
    }
}

/// Silent reporter: emits nothing.
pub struct SilentReporter;

impl StatusReporter for SilentReporter {
    fn report_stage(&self, _current: u32, _total: u32, _description: &str) {}
    fn report_complete(&self, _tool_key: &'static str) {}
}

#[derive(Debug)]
struct IndicatifState {
    progress: MultiProgress,
    bars: Vec<ProgressBar>,
    descriptions: Vec<String>,
    running_index: Option<usize>,
    completed: bool,
    is_hidden: bool,
}

/// Standard reporter backed by `indicatif::MultiProgress`.
pub struct IndicatifReporter {
    state: Mutex<IndicatifState>,
}

impl IndicatifReporter {
    /// Construct an `indicatif` reporter with one persistent line per stage.
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
                keys::STATUS_STATE_PENDING,
                current,
                PIPELINE_STAGE_COUNT,
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
        status_key: &'static str,
        finish_line: bool,
    ) {
        let Ok(current) = u32::try_from(index + 1) else {
            return;
        };
        let description = state
            .descriptions
            .get(index)
            .cloned()
            .unwrap_or_else(String::new);
        let message = stage_summary(status_key, current, PIPELINE_STAGE_COUNT, &description);
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
            Self::set_stage_state(&mut state, index, keys::STATUS_STATE_FAILED, true);
        }
        let _ = &state.progress;
    }
}

impl StatusReporter for IndicatifReporter {
    fn report_stage(&self, current: u32, _total: u32, description: &str) {
        let Ok(index) = usize::try_from(current.saturating_sub(1)) else {
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
        description.clone_into(existing_description);
        if let Some(previous) = state.running_index
            && previous != index
        {
            Self::set_stage_state(&mut state, previous, keys::STATUS_STATE_DONE, true);
        }

        Self::set_stage_state(&mut state, index, keys::STATUS_STATE_RUNNING, false);
        state.running_index = Some(index);
    }

    fn report_complete(&self, tool_key: &'static str) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(index) = state.running_index.take() {
            Self::set_stage_state(&mut state, index, keys::STATUS_STATE_DONE, true);
        }
        state.completed = true;
        let _ = &state.progress;

        let tool = localization::message(tool_key);
        let message = localization::message(keys::STATUS_COMPLETE).with_arg("tool", tool);
        drop(writeln!(io::stderr(), "{message}"));
    }
}

/// Enumerates the known pipeline stages in reporting order.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PipelineStage {
    /// Stage 1: read the manifest from disk.
    ManifestIngestion = 1,
    /// Stage 2: parse raw YAML into an intermediate tree.
    InitialYamlParsing = 2,
    /// Stage 3: expand `foreach` and `when` template directives.
    TemplateExpansion = 3,
    /// Stage 4: deserialize and render manifest values.
    FinalRendering = 4,
    /// Stage 5: build and validate the dependency graph.
    IrGenerationValidation = 5,
    /// Stage 6: synthesize Ninja and execute the selected tool.
    NinjaSynthesisAndExecution = 6,
}

impl PipelineStage {
    /// All pipeline stages in reporting order.
    pub const ALL: [Self; 6] = [
        Self::ManifestIngestion,
        Self::InitialYamlParsing,
        Self::TemplateExpansion,
        Self::FinalRendering,
        Self::IrGenerationValidation,
        Self::NinjaSynthesisAndExecution,
    ];

    /// 1-based index of this stage within the pipeline.
    #[must_use]
    pub const fn index(self) -> u32 {
        self as u32
    }

    /// Convert a 1-based stage index into a [`PipelineStage`].
    #[must_use]
    pub const fn from_index(index: u32) -> Option<Self> {
        match index {
            1 => Some(Self::ManifestIngestion),
            2 => Some(Self::InitialYamlParsing),
            3 => Some(Self::TemplateExpansion),
            4 => Some(Self::FinalRendering),
            5 => Some(Self::IrGenerationValidation),
            6 => Some(Self::NinjaSynthesisAndExecution),
            _ => None,
        }
    }

    /// Localized description of this stage.
    #[must_use]
    pub fn description(self, tool_key: Option<&'static str>) -> String {
        match self {
            Self::ManifestIngestion => {
                localization::message(keys::STATUS_STAGE_MANIFEST_INGESTION).to_string()
            }
            Self::InitialYamlParsing => {
                localization::message(keys::STATUS_STAGE_INITIAL_YAML_PARSING).to_string()
            }
            Self::TemplateExpansion => {
                localization::message(keys::STATUS_STAGE_TEMPLATE_EXPANSION).to_string()
            }
            Self::FinalRendering => {
                localization::message(keys::STATUS_STAGE_FINAL_RENDERING).to_string()
            }
            Self::IrGenerationValidation => {
                localization::message(keys::STATUS_STAGE_IR_GENERATION_VALIDATION).to_string()
            }
            Self::NinjaSynthesisAndExecution => tool_key.map_or_else(
                || localization::message(keys::STATUS_STAGE_NINJA_SYNTHESIS).to_string(),
                |tool_message_key| {
                    let tool = localization::message(tool_message_key).to_string();
                    localization::message(keys::STATUS_STAGE_NINJA_SYNTHESIS_EXECUTE)
                        .with_arg("tool", tool)
                        .to_string()
                },
            ),
        }
    }
}

/// The total number of pipeline stages reported during a build.
pub const PIPELINE_STAGE_COUNT: u32 = 6;

/// Report a pipeline stage via a [`StatusReporter`].
pub fn report_pipeline_stage(
    reporter: &dyn StatusReporter,
    stage: PipelineStage,
    tool_key: Option<&'static str>,
) {
    reporter.report_stage(
        stage.index(),
        PIPELINE_STAGE_COUNT,
        &stage.description(tool_key),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn pipeline_stage_count_matches_stage_array() {
        let stage_count = u32::try_from(PipelineStage::ALL.len()).unwrap_or(0);
        assert_eq!(PIPELINE_STAGE_COUNT, stage_count);
    }

    #[rstest]
    #[case(PipelineStage::ManifestIngestion, 1)]
    #[case(PipelineStage::InitialYamlParsing, 2)]
    #[case(PipelineStage::TemplateExpansion, 3)]
    #[case(PipelineStage::FinalRendering, 4)]
    #[case(PipelineStage::IrGenerationValidation, 5)]
    #[case(PipelineStage::NinjaSynthesisAndExecution, 6)]
    fn stage_index_round_trips(#[case] stage: PipelineStage, #[case] expected: u32) {
        assert_eq!(stage.index(), expected);
        assert_eq!(PipelineStage::from_index(expected), Some(stage));
    }

    #[test]
    fn invalid_stage_index_returns_none() {
        assert_eq!(PipelineStage::from_index(0), None);
        assert_eq!(PipelineStage::from_index(7), None);
    }
}
