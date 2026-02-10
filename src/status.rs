//! Pipeline status reporting for accessible and standard output modes.
//!
//! This module provides a [`StatusReporter`] trait and concrete
//! implementations that emit progress feedback during Netsuke's build
//! pipeline. [`AccessibleReporter`] writes static, labelled status lines
//! to stderr suitable for screen readers and dumb terminals.
//! [`SilentReporter`] emits nothing, serving as the default until future
//! animated progress indicators are added.

use crate::localization::{self, keys};
use std::io::{self, Write};

/// Report pipeline progress to the user.
///
/// Implementations decide how (or whether) to present stage transitions
/// and completion to the user. The trait is object-safe so callers can
/// dispatch dynamically based on the resolved output mode.
pub trait StatusReporter {
    /// Emit a status line for the given pipeline stage.
    fn report_stage(&self, current: u32, total: u32, description: &str);

    /// Emit a completion message after a successful pipeline run.
    ///
    /// The `tool_key` is a Fluent message key identifying the tool name
    /// (e.g., [`keys::STATUS_TOOL_BUILD`]).
    fn report_complete(&self, tool_key: &'static str);
}

/// Accessible reporter: writes static, labelled lines to stderr.
///
/// Each line follows the pattern `Stage N/M: Description`, using
/// localized messages from the Fluent resource bundle.
pub struct AccessibleReporter;

impl StatusReporter for AccessibleReporter {
    fn report_stage(&self, current: u32, total: u32, description: &str) {
        let message = localization::message(keys::STATUS_STAGE_LABEL)
            .with_arg("current", current.to_string())
            .with_arg("total", total.to_string())
            .with_arg("description", description);
        // Intentionally discard the write result: a failed status line
        // should not abort the build pipeline.
        drop(writeln!(io::stderr(), "{message}"));
    }

    fn report_complete(&self, tool_key: &'static str) {
        let tool = localization::message(tool_key);
        let message = localization::message(keys::STATUS_COMPLETE).with_arg("tool", tool);
        // Intentionally discard the write result (see above).
        drop(writeln!(io::stderr(), "{message}"));
    }
}

/// Silent reporter: emits nothing.
///
/// Used in standard output mode until future work (roadmap 3.9) adds
/// animated progress indicators via `indicatif`.
pub struct SilentReporter;

impl StatusReporter for SilentReporter {
    fn report_stage(&self, _current: u32, _total: u32, _description: &str) {}
    fn report_complete(&self, _tool_key: &'static str) {}
}

/// The total number of pipeline stages reported during a build.
pub const PIPELINE_STAGE_COUNT: u32 = 5;

/// Enumerates the known pipeline stages in the order they are reported.
///
/// Keeping stage indices and descriptions centralized here avoids
/// hard-coded literals at call sites and ensures [`PIPELINE_STAGE_COUNT`]
/// stays consistent with the stages that are reported.
#[derive(Copy, Clone, Debug)]
pub enum PipelineStage {
    /// Stage 1: configuring the network policy.
    NetworkPolicy = 1,
    /// Stage 2: loading the manifest.
    ManifestLoad = 2,
    /// Stage 3: building the dependency graph.
    BuildGraph = 3,
    /// Stage 4: generating the Ninja file.
    GenerateNinja = 4,
    /// Stage 5: executing the build.
    Execute = 5,
}

impl PipelineStage {
    /// 1-based index of this stage within the pipeline.
    #[must_use]
    pub const fn index(self) -> u32 {
        self as u32
    }

    /// Localized description of this stage.
    ///
    /// For [`PipelineStage::Execute`], the description includes a
    /// `{$tool}` placeholder resolved from the provided `tool_key`. Pass
    /// [`None`] for non-Execute stages.
    #[must_use]
    pub fn description(self, tool_key: Option<&'static str>) -> String {
        match self {
            Self::NetworkPolicy => {
                localization::message(keys::STATUS_STAGE_NETWORK_POLICY).to_string()
            }
            Self::ManifestLoad => {
                localization::message(keys::STATUS_STAGE_MANIFEST_LOAD).to_string()
            }
            Self::BuildGraph => localization::message(keys::STATUS_STAGE_BUILD_GRAPH).to_string(),
            Self::GenerateNinja => {
                localization::message(keys::STATUS_STAGE_GENERATE_NINJA).to_string()
            }
            Self::Execute => {
                let tool =
                    tool_key.map_or_else(String::new, |k| localization::message(k).to_string());
                localization::message(keys::STATUS_STAGE_EXECUTE)
                    .with_arg("tool", tool)
                    .to_string()
            }
        }
    }
}

/// Report a pipeline stage via a [`StatusReporter`].
///
/// Centralizes the use of [`PIPELINE_STAGE_COUNT`] so call sites do not
/// need to know the numeric indices or total stage count. Pass a
/// `tool_key` for the [`PipelineStage::Execute`] stage to produce a
/// tool-specific description (e.g., "Executing clean"); other stages
/// ignore it.
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
