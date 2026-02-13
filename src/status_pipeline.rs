//! Pipeline stage model and stage reporting helper.

use super::{LocalizationKey, PIPELINE_STAGE_TOTAL, StageDescription, StageNumber, StatusReporter};
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
        StageNumber(self as u32)
    }

    /// Map a raw stage index to a stage enum variant.
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

    /// Return the localized description for this stage.
    #[must_use]
    pub fn description(self, tool_key: Option<LocalizationKey>) -> StageDescription {
        match self {
            Self::ManifestIngestion => StageDescription::new(
                localization::message(keys::STATUS_STAGE_MANIFEST_INGESTION).to_string(),
            ),
            Self::InitialYamlParsing => StageDescription::new(
                localization::message(keys::STATUS_STAGE_INITIAL_YAML_PARSING).to_string(),
            ),
            Self::TemplateExpansion => StageDescription::new(
                localization::message(keys::STATUS_STAGE_TEMPLATE_EXPANSION).to_string(),
            ),
            Self::FinalRendering => StageDescription::new(
                localization::message(keys::STATUS_STAGE_FINAL_RENDERING).to_string(),
            ),
            Self::IrGenerationValidation => StageDescription::new(
                localization::message(keys::STATUS_STAGE_IR_GENERATION_VALIDATION).to_string(),
            ),
            Self::NinjaSynthesisAndExecution => tool_key.map_or_else(
                || {
                    StageDescription::new(
                        localization::message(keys::STATUS_STAGE_NINJA_SYNTHESIS).to_string(),
                    )
                },
                |tool_message_key| {
                    let tool = localization::message(tool_message_key.as_str()).to_string();
                    StageDescription::new(
                        localization::message(keys::STATUS_STAGE_NINJA_SYNTHESIS_EXECUTE)
                            .with_arg("tool", tool)
                            .to_string(),
                    )
                },
            ),
        }
    }
}

/// Emit a localized status update for a concrete pipeline stage.
pub fn report_pipeline_stage(
    reporter: &dyn StatusReporter,
    stage: PipelineStage,
    tool_key: Option<LocalizationKey>,
) {
    reporter.report_stage(
        stage.index(),
        PIPELINE_STAGE_TOTAL,
        stage.description(tool_key),
    );
}
