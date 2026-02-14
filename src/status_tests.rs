//! Tests for status stage modelling and index conversions.

use super::*;
use rstest::rstest;

#[rstest]
#[case(PipelineStage::ManifestIngestion, 1)]
#[case(PipelineStage::InitialYamlParsing, 2)]
#[case(PipelineStage::TemplateExpansion, 3)]
#[case(PipelineStage::FinalRendering, 4)]
#[case(PipelineStage::IrGenerationValidation, 5)]
#[case(PipelineStage::NinjaSynthesisAndExecution, 6)]
fn stage_index_matches_discriminant(#[case] stage: PipelineStage, #[case] expected: u32) {
    assert_eq!(stage.index().get(), expected);
}

#[test]
fn pipeline_stage_count_matches_stage_array() {
    let stage_count = u32::try_from(PipelineStage::ALL.len()).expect("stage array length fits u32");
    assert_eq!(PIPELINE_STAGE_COUNT, stage_count);
}

#[test]
fn localization_key_round_trips_static_key() {
    let key = LocalizationKey::new(keys::STATUS_STATE_PENDING);
    assert_eq!(key.as_str(), keys::STATUS_STATE_PENDING);
}

#[test]
fn localization_key_from_static_str() {
    let key: LocalizationKey = keys::STATUS_STATE_PENDING.into();
    assert_eq!(key.as_str(), keys::STATUS_STATE_PENDING);
}
