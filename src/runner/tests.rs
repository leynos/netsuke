//! Unit tests for the runner module's helpers.

use super::*;
use crate::status::{PIPELINE_STAGE_COUNT, report_execute_stage, report_pipeline_stage};
use rstest::rstest;
use std::cell::RefCell;
use std::path::PathBuf;

#[rstest]
#[case(None, "out.ninja", "out.ninja")]
#[case(Some("work"), "out.ninja", "work/out.ninja")]
#[case(Some("work"), "/tmp/out.ninja", "/tmp/out.ninja")]
fn resolve_output_path_respects_directory(
    #[case] directory: Option<&str>,
    #[case] input: &str,
    #[case] expected: &str,
) {
    let cli = Cli {
        directory: directory.map(PathBuf::from),
        ..Cli::default()
    };
    let resolved = resolve_output_path(&cli, Path::new(input));
    assert_eq!(resolved.as_ref(), Path::new(expected));
}

// ---------------------------------------------------------------------------
// Status-reporting integration tests
// ---------------------------------------------------------------------------

/// Recorded event emitted by [`RecordingReporter`].
#[derive(Debug, PartialEq)]
enum ReporterEvent {
    Stage { current: u32, total: u32 },
    Complete { tool_key: &'static str },
}

/// Test reporter that records all events for later assertion.
#[derive(Default)]
struct RecordingReporter {
    events: RefCell<Vec<ReporterEvent>>,
}

impl StatusReporter for RecordingReporter {
    fn report_stage(&self, current: u32, total: u32, _description: &str) {
        self.events
            .borrow_mut()
            .push(ReporterEvent::Stage { current, total });
    }

    fn report_complete(&self, tool_key: &'static str) {
        self.events
            .borrow_mut()
            .push(ReporterEvent::Complete { tool_key });
    }
}

#[rstest]
fn accessible_flag_selects_accessible_mode() {
    let mode = output_mode::resolve(Some(true));
    assert!(
        mode.is_accessible(),
        "explicit accessible=true should resolve to Accessible"
    );
}

#[rstest]
fn no_accessible_flag_selects_standard_mode() {
    let mode = output_mode::resolve(Some(false));
    assert!(
        !mode.is_accessible(),
        "explicit accessible=false should resolve to Standard"
    );
}

#[rstest]
fn report_pipeline_stage_emits_correct_index_and_total() {
    let recorder = RecordingReporter::default();
    report_pipeline_stage(&recorder, PipelineStage::NetworkPolicy);
    report_pipeline_stage(&recorder, PipelineStage::ManifestLoad);
    report_pipeline_stage(&recorder, PipelineStage::BuildGraph);
    report_pipeline_stage(&recorder, PipelineStage::GenerateNinja);

    let events = recorder.events.borrow();
    let expected = vec![
        ReporterEvent::Stage {
            current: 1,
            total: PIPELINE_STAGE_COUNT,
        },
        ReporterEvent::Stage {
            current: 2,
            total: PIPELINE_STAGE_COUNT,
        },
        ReporterEvent::Stage {
            current: 3,
            total: PIPELINE_STAGE_COUNT,
        },
        ReporterEvent::Stage {
            current: 4,
            total: PIPELINE_STAGE_COUNT,
        },
    ];
    assert_eq!(*events, expected);
}

#[rstest]
fn report_execute_stage_emits_stage_five() {
    let recorder = RecordingReporter::default();
    report_execute_stage(&recorder, keys::STATUS_TOOL_BUILD);

    let events = recorder.events.borrow();
    assert_eq!(
        *events,
        vec![ReporterEvent::Stage {
            current: PipelineStage::Execute.index(),
            total: PIPELINE_STAGE_COUNT,
        }]
    );
}

#[rstest]
fn report_complete_records_tool_key() {
    let recorder = RecordingReporter::default();
    recorder.report_complete(keys::STATUS_TOOL_BUILD);

    let events = recorder.events.borrow();
    assert_eq!(
        *events,
        vec![ReporterEvent::Complete {
            tool_key: keys::STATUS_TOOL_BUILD,
        }]
    );
}
