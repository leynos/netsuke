//! Tests for verbose timing summary support.

use super::*;
use rstest::rstest;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn strip_isolates(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !matches!(ch, '\u{2068}' | '\u{2069}'))
        .collect()
}

#[derive(Debug)]
struct FakeClock {
    values: Mutex<VecDeque<Duration>>,
    fallback: Duration,
    call_count: AtomicUsize,
}

impl FakeClock {
    fn from_millis(values: &[u64]) -> Self {
        let points = values
            .iter()
            .copied()
            .map(Duration::from_millis)
            .collect::<VecDeque<_>>();
        let fallback = points.back().copied().unwrap_or(Duration::ZERO);
        Self {
            values: Mutex::new(points),
            fallback,
            call_count: AtomicUsize::new(0),
        }
    }

    fn now(&self) -> Duration {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let mut values = self
            .values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        values.pop_front().unwrap_or(self.fallback)
    }

    fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[rstest]
fn timing_recorder_renders_happy_path_summary() {
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

    let lines = render_summary_lines(state.completed_stages());
    let [header, stage1, stage2, stage3, total_line] = lines.as_slice() else {
        panic!("expected 5 timing summary lines");
    };
    assert_eq!(strip_isolates(header), "Stage timing summary:");
    assert_eq!(
        strip_isolates(stage1),
        "- Stage 1/6: Reading manifest file: 12ms"
    );
    assert_eq!(
        strip_isolates(stage2),
        "- Stage 2/6: Parsing YAML document: 4ms"
    );
    assert_eq!(
        strip_isolates(stage3),
        "- Stage 3/6: Expanding template directives: 7ms"
    );
    assert_eq!(strip_isolates(total_line), "Total pipeline time: 23ms");
}

#[rstest]
fn timing_recorder_incomplete_flow_has_no_summary_lines() {
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

    let lines = render_summary_lines(state.completed_stages());
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
fn verbose_timing_reporter_finalizes_current_stage_on_complete() {
    struct ObservingReporter {
        observed_clock_calls: Arc<Mutex<Vec<usize>>>,
        clock: Arc<FakeClock>,
    }

    impl StatusReporter for ObservingReporter {
        fn report_stage(&self, _current: StageNumber, _total: StageNumber, _description: &str) {}
        fn report_complete(&self, _tool_key: LocalizationKey) {
            let mut observed = self
                .observed_clock_calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            observed.push(self.clock.call_count());
        }
    }

    let observed_clock_calls = Arc::new(Mutex::new(Vec::new()));
    let clock = Arc::new(FakeClock::from_millis(&[0, 15]));
    let reporter_clock = Arc::clone(&clock);
    let reporter = VerboseTimingReporter::with_clock(
        Box::new(ObservingReporter {
            observed_clock_calls: Arc::clone(&observed_clock_calls),
            clock: Arc::clone(&clock),
        }),
        Box::new(move || reporter_clock.now()),
    );
    reporter.report_stage(
        StageNumber::new_unchecked(1),
        StageNumber::new_unchecked(6),
        "Reading manifest file",
    );
    reporter.report_complete(LocalizationKey::new(keys::STATUS_TOOL_MANIFEST));

    let observed = observed_clock_calls
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(
        observed.as_slice(),
        &[2],
        "stage timing should be finalized before inner completion output"
    );

    let state = reporter
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let lines = render_summary_lines(state.completed_stages());
    let [header, stage_line, total_line] = lines.as_slice() else {
        panic!("expected 3 timing summary lines");
    };
    assert_eq!(strip_isolates(header), "Stage timing summary:");
    assert!(strip_isolates(stage_line).contains("Stage 1/6: Reading manifest file"));
    assert!(strip_isolates(stage_line).ends_with(": 15ms"));
    assert_eq!(strip_isolates(total_line), "Total pipeline time: 15ms");
}

#[rstest]
fn verbose_timing_reporter_suppresses_progress_updates_after_complete() {
    #[derive(Debug, Default)]
    struct Counts {
        stages: usize,
        tasks: usize,
        completions: usize,
    }

    struct CountingReporter {
        counts: Arc<Mutex<Counts>>,
    }

    impl StatusReporter for CountingReporter {
        fn report_stage(&self, _current: StageNumber, _total: StageNumber, _description: &str) {
            let mut counts = self
                .counts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            counts.stages += 1;
        }

        fn report_task_progress(&self, _current: u32, _total: u32, _description: &str) {
            let mut counts = self
                .counts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            counts.tasks += 1;
        }

        fn report_complete(&self, _tool_key: LocalizationKey) {
            let mut counts = self
                .counts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            counts.completions += 1;
        }
    }

    let counts = Arc::new(Mutex::new(Counts::default()));
    let reporter = VerboseTimingReporter::with_clock(
        Box::new(CountingReporter {
            counts: Arc::clone(&counts),
        }),
        Box::new(|| Duration::from_millis(50)),
    );
    reporter.report_stage(
        StageNumber::new_unchecked(1),
        StageNumber::new_unchecked(6),
        "Reading manifest file",
    );
    reporter.report_task_progress(1, 2, "cc -c src/main.c");
    reporter.report_complete(LocalizationKey::new(keys::STATUS_TOOL_MANIFEST));
    reporter.report_stage(
        StageNumber::new_unchecked(2),
        StageNumber::new_unchecked(6),
        "Parsing YAML document",
    );
    reporter.report_task_progress(2, 2, "cc -c src/lib.c");

    let final_counts = counts
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(
        final_counts.stages, 1,
        "stage updates should stop after completion"
    );
    assert_eq!(
        final_counts.tasks, 1,
        "task updates should stop after completion"
    );
    assert_eq!(
        final_counts.completions, 1,
        "completion should still be delegated"
    );
}
