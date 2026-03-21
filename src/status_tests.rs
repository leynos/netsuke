//! Tests for status stage modelling and index conversions.

use super::*;
use crate::output_prefs;
use rstest::{fixture, rstest};

fn test_prefs() -> crate::output_prefs::OutputPrefs {
    output_prefs::resolve_with(None, |_| None)
}

fn strip_isolates(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !matches!(ch, '\u{2068}' | '\u{2069}'))
        .collect()
}

fn stage6_message(reporter: &IndicatifReporter) -> String {
    let state = reporter
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state
        .bars
        .get(STAGE6_INDEX)
        .expect("stage 6 progress bar should exist")
        .message()
}

#[fixture]
fn force_text_reporter() -> IndicatifReporter {
    IndicatifReporter::with_force_text_task_updates(test_prefs(), true)
}

#[fixture]
fn running_stage6_reporter() -> IndicatifReporter {
    let reporter = IndicatifReporter::with_force_text_task_updates(test_prefs(), false);
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
    reporter
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

#[rstest]
fn indicatif_reporter_ignores_task_updates_when_stage6_is_not_running(
    force_text_reporter: IndicatifReporter,
) {
    force_text_reporter.report_task_progress(1, 2, "cc -c src/a.c");
    let stage6_message = stage6_message(&force_text_reporter);
    assert!(!strip_isolates(&stage6_message).contains("Task 1/2"));
}

#[rstest]
fn indicatif_reporter_sets_stage6_bar_message_for_non_text_updates(
    running_stage6_reporter: IndicatifReporter,
) {
    running_stage6_reporter.report_task_progress(1, 2, "cc -c src/a.c");
    let stage6_message = stage6_message(&running_stage6_reporter);
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

#[rstest]
fn accessible_reporter_formats_stage_with_info_prefix() {
    let prefs = test_prefs();
    let reporter = AccessibleReporter::with_writer(prefs, Vec::new());
    reporter.report_stage(
        StageNumber::new_unchecked(1),
        StageNumber::new_unchecked(6),
        "Reading manifest file",
    );

    let output = reporter
        .writer
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let line = strip_isolates(&String::from_utf8_lossy(&output));
    let info_prefix = strip_isolates(&prefs.info_prefix().to_string());
    assert!(
        line.starts_with(&info_prefix),
        "stage line should start with info prefix; line was: {line:?}, prefix was: {info_prefix:?}"
    );
    assert!(
        line.contains("Stage 1/6: Reading manifest file"),
        "stage line should contain the stage label; line was: {line:?}"
    );
}

#[rstest]
fn accessible_reporter_indents_task_progress() {
    let prefs = test_prefs();
    let reporter = AccessibleReporter::with_writer(prefs, Vec::new());
    reporter.report_task_progress(1, 2, "cc -c src/main.c");

    let output = reporter
        .writer
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let line = strip_isolates(&String::from_utf8_lossy(&output));
    let info_prefix = strip_isolates(&prefs.info_prefix().to_string());
    assert!(
        line.starts_with(prefs.task_indent()),
        "task line should be indented by two spaces; line was: {line:?}"
    );
    assert!(
        !line.trim_start().starts_with(&info_prefix),
        "task line should not include info prefix; line was: {line:?}, prefix was: {info_prefix:?}"
    );
}

#[rstest]
fn completion_line_includes_success_prefix() {
    let prefs = test_prefs();
    let line = strip_isolates(&format_completion_line(
        prefs,
        LocalizationKey::new(keys::STATUS_TOOL_MANIFEST),
    ));
    let success_prefix = strip_isolates(&prefs.success_prefix().to_string());
    assert!(
        line.starts_with(&success_prefix),
        "completion line should start with success prefix; line was: {line:?}, prefix was: {success_prefix:?}"
    );
}
