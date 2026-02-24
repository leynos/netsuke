//! Tests for status stage modelling and index conversions.

use super::*;
use rstest::rstest;

fn strip_isolates(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !matches!(ch, '\u{2068}' | '\u{2069}'))
        .collect()
}

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
fn pipeline_stage_total_derived_from_all() {
    let expected = u32::try_from(PipelineStage::ALL.len()).expect("stage array length fits u32");
    assert_eq!(PIPELINE_STAGE_TOTAL.get(), expected);
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

#[rstest]
#[case(1, 2, "cc -c src/main.c", "Task 1/2: cc -c src/main.c")]
#[case(2, 2, "", "Task 2/2")]
fn task_progress_update_formats_expected_text(
    #[case] current: u32,
    #[case] total: u32,
    #[case] description: &str,
    #[case] expected: &str,
) {
    let rendered = task_progress_update(current, total, description);
    assert_eq!(strip_isolates(&rendered), expected);
}

#[test]
fn indicatif_reporter_ignores_task_updates_when_stage6_is_not_running() {
    let reporter = IndicatifReporter::new(true);
    reporter.report_task_progress(1, 2, "cc -c src/a.c");

    let state = reporter
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let stage6_message = state
        .bars
        .get(STAGE6_INDEX)
        .expect("stage 6 progress bar should exist")
        .message()
        .to_string();
    assert!(!strip_isolates(&stage6_message).contains("Task 1/2"));
}

#[test]
fn indicatif_reporter_sets_stage6_bar_message_for_non_text_updates() {
    let reporter = IndicatifReporter::new(false);
    {
        let mut state = reporter
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.is_hidden = false;
    }
    reporter.report_stage(
        PipelineStage::NinjaSynthesisAndExecution.index(),
        PIPELINE_STAGE_TOTAL,
        "Executing Build",
    );
    reporter.report_task_progress(1, 2, "cc -c src/a.c");

    let state = reporter
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let stage6_message = state
        .bars
        .get(STAGE6_INDEX)
        .expect("stage 6 progress bar should exist")
        .message()
        .to_string();
    let task = task_progress_update(1, 2, "cc -c src/a.c");
    let state_label = localization::message(keys::STATUS_STATE_RUNNING).to_string();
    let stage_line = stage_label(
        PipelineStage::NinjaSynthesisAndExecution.index(),
        PIPELINE_STAGE_TOTAL,
        "Executing Build",
    );
    let expected = localization::message(keys::STATUS_STAGE_SUMMARY_WITH_TASK)
        .with_arg("state", state_label)
        .with_arg("label", stage_line)
        .with_arg("task_progress", &task)
        .to_string();
    assert_eq!(strip_isolates(&stage6_message), strip_isolates(&expected));
}
