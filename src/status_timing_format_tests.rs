//! Formatting tests for verbose timing summaries.

use super::*;
use crate::output_prefs;
use crate::snapshot_test_support::{snapshot_settings, theme_prefs};
use insta::assert_snapshot;
use rstest::{fixture, rstest};
use test_support::fluent::normalize_fluent_isolates;
use test_support::{EnLocalizer, en_localizer};

#[fixture]
fn test_prefs() -> OutputPrefs {
    output_prefs::resolve_with(None, |_| None)
}

#[rstest]
fn timing_recorder_renders_happy_path_summary(test_prefs: OutputPrefs) {
    let total = StageNumber::new_unchecked(6);
    let mut state = TimingState::default();
    state.start_stage(
        Duration::from_millis(0),
        StageMarker {
            current: StageNumber::new_unchecked(1),
            total,
        },
        "Reading manifest file",
    );
    state.start_stage(
        Duration::from_millis(12),
        StageMarker {
            current: StageNumber::new_unchecked(2),
            total,
        },
        "Parsing YAML document",
    );
    state.start_stage(
        Duration::from_millis(16),
        StageMarker {
            current: StageNumber::new_unchecked(3),
            total,
        },
        "Expanding template directives",
    );
    state.finish(Duration::from_millis(23));

    let lines = render_summary_lines(test_prefs, state.completed_stages());
    let [header, stage1, stage2, stage3, total_line] = lines.as_slice() else {
        panic!("expected 5 timing summary lines");
    };
    assert!(normalize_fluent_isolates(header).contains("Timing:"));
    assert!(normalize_fluent_isolates(header).contains("Stage timing summary:"));
    assert_eq!(
        normalize_fluent_isolates(stage1),
        "  - Stage 1/6: Reading manifest file: 12ms"
    );
    assert_eq!(
        normalize_fluent_isolates(stage2),
        "  - Stage 2/6: Parsing YAML document: 4ms"
    );
    assert_eq!(
        normalize_fluent_isolates(stage3),
        "  - Stage 3/6: Expanding template directives: 7ms"
    );
    assert_eq!(
        normalize_fluent_isolates(total_line),
        "  Total pipeline time: 23ms"
    );
}

#[rstest]
fn timing_recorder_incomplete_flow_has_no_summary_lines(test_prefs: OutputPrefs) {
    let total = StageNumber::new_unchecked(6);
    let mut state = TimingState::default();
    state.start_stage(
        Duration::from_millis(0),
        StageMarker {
            current: StageNumber::new_unchecked(1),
            total,
        },
        "Reading manifest file",
    );

    let lines = render_summary_lines(test_prefs, state.completed_stages());
    assert!(lines.is_empty());
}

#[rstest]
#[case(Duration::from_nanos(7), "7ns")]
#[case(Duration::from_micros(18), "18us")]
#[case(Duration::from_millis(22), "22ms")]
#[case(Duration::from_millis(1_900), "1.900s")]
#[case(Duration::from_secs(3), "3s")]
fn duration_formatting_uses_expected_units(#[case] duration: Duration, #[case] expected: &str) {
    assert_eq!(format_duration(duration), expected);
}

#[rstest]
#[case::unicode(crate::theme::ThemePreference::Unicode, "timing_summary_unicode")]
#[case::ascii(crate::theme::ThemePreference::Ascii, "timing_summary_ascii")]
fn timing_summary_snapshot(
    en_localizer: EnLocalizer,
    #[case] theme: crate::theme::ThemePreference,
    #[case] snapshot_name: &str,
) {
    let _localizer = en_localizer;
    let total = StageNumber::new_unchecked(6);
    let mut state = TimingState::default();
    state.start_stage(
        Duration::from_millis(0),
        StageMarker {
            current: StageNumber::new_unchecked(1),
            total,
        },
        "Reading manifest file",
    );
    state.start_stage(
        Duration::from_millis(12),
        StageMarker {
            current: StageNumber::new_unchecked(2),
            total,
        },
        "Parsing YAML document",
    );
    state.start_stage(
        Duration::from_millis(16),
        StageMarker {
            current: StageNumber::new_unchecked(3),
            total,
        },
        "Expanding template directives",
    );
    state.finish(Duration::from_millis(23));

    let rendered = normalize_fluent_isolates(
        &render_summary_lines(theme_prefs(theme), state.completed_stages()).join("\n"),
    );

    snapshot_settings("status_timing").bind(|| {
        assert_snapshot!(snapshot_name, rendered);
    });
}
