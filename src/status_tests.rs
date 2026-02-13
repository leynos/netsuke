//! Tests for status stage modelling and index conversions.

use super::*;
use rstest::rstest;

#[rstest]
#[case(1)]
#[case(PIPELINE_STAGE_COUNT)]
fn stage_number_accepts_in_range_values(#[case] value: u32) {
    let stage = StageNumber::new(value).expect("in-range stage number should be valid");
    assert_eq!(stage.get(), value);
}

#[rstest]
#[case(0)]
#[case(PIPELINE_STAGE_COUNT + 1)]
#[case(u32::MAX)]
fn stage_number_rejects_out_of_range_values(#[case] value: u32) {
    let error = StageNumber::new(value).expect_err("out-of-range stage number should fail");
    assert_eq!(error, StageNumberError::OutOfRange(value));
}

#[test]
fn stage_description_round_trips_string_content() {
    let description = StageDescription::new(String::from("rendering"));
    assert_eq!(description.as_str(), "rendering");

    let from_impl: StageDescription = String::from("building").into();
    assert_eq!(from_impl.as_str(), "building");
}

#[test]
fn localization_key_round_trips_static_key() {
    let key = LocalizationKey::new(keys::STATUS_STATE_PENDING);
    assert_eq!(key.as_str(), keys::STATUS_STATE_PENDING);
}

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
    assert_eq!(stage.index().get(), expected);
    assert_eq!(PipelineStage::from_index(expected), Some(stage));
}

#[test]
fn invalid_stage_index_returns_none() {
    assert_eq!(PipelineStage::from_index(0), None);
    assert_eq!(PipelineStage::from_index(7), None);
}
