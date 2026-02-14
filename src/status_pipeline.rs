//! Pipeline stage model and stage reporting helper.

use super::{LocalizationKey, PIPELINE_STAGE_TOTAL, StageNumber, StatusReporter};
use crate::localization::{self, keys};

/// Enumerates pipeline stages in user-visible execution order.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PipelineStage {
    /// Stage 1: read manifest content from disk.
    ManifestIngestion = 1,
    /// Stage 2: parse the YAML document.
    InitialYamlParsing = 2,
    /// Stage 3: expand templated target directives.
    TemplateExpansion = 3,
    /// Stage 4: finalize manifest rendering.
    FinalRendering = 4,
    /// Stage 5: build and validate dependency IR.
    IrGenerationValidation = 5,
    /// Stage 6: synthesize Ninja and execute command intent.
    NinjaSynthesisAndExecution = 6,
}

impl PipelineStage {
    /// All stages in pipeline order.
    pub const ALL: [Self; 6] = [
        Self::ManifestIngestion,
        Self::InitialYamlParsing,
        Self::TemplateExpansion,
        Self::FinalRendering,
        Self::IrGenerationValidation,
        Self::NinjaSynthesisAndExecution,
    ];

    /// Return the validated stage index for this variant.
    #[must_use]
    pub const fn index(self) -> StageNumber {
        StageNumber::new_unchecked(self as u32)
    }

    /// Return the localized description for this stage.
    #[must_use]
    pub fn description(self, tool_key: Option<LocalizationKey>) -> String {
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
                    let tool = localization::message(tool_message_key.as_str()).to_string();
                    localization::message(keys::STATUS_STAGE_NINJA_SYNTHESIS_EXECUTE)
                        .with_arg("tool", tool)
                        .to_string()
                },
            ),
        }
    }
}

/// Compile-time guard ensuring `PipelineStage::ALL` stays in sync with the
/// declared `PIPELINE_STAGE_COUNT`.
const _: () = assert!(
    PipelineStage::ALL.len() == super::PIPELINE_STAGE_COUNT as usize,
    "PipelineStage::ALL length must equal PIPELINE_STAGE_COUNT"
);

/// Emit a localized status update for a concrete pipeline stage.
pub fn report_pipeline_stage(
    reporter: &dyn StatusReporter,
    stage: PipelineStage,
    tool_key: Option<LocalizationKey>,
) {
    let description = stage.description(tool_key);
    reporter.report_stage(stage.index(), PIPELINE_STAGE_TOTAL, &description);
}
