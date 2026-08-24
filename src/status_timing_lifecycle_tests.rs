//! Lifecycle tests for verbose timing summaries.

use super::*;
use crate::output_prefs;
use rstest::{fixture, rstest};
use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use test_support::fluent::normalize_fluent_isolates;
use test_support::{EnLocalizer, en_localizer};

#[fixture]
fn test_prefs() -> OutputPrefs {
    output_prefs::resolve_with(None, |_| None)
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
        self.values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front()
            .unwrap_or(self.fallback)
    }

    fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[derive(Clone, Debug)]
struct SharedBufferWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl SharedBufferWriter {
    fn new(buffer: Arc<Mutex<Vec<u8>>>) -> Self {
        Self { buffer }
    }
}

impl Write for SharedBufferWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.buffer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[rstest]
fn verbose_timing_reporter_finalizes_current_stage_on_complete(test_prefs: OutputPrefs) {
    struct ObservingReporter {
        observed_clock_calls: Arc<Mutex<Vec<usize>>>,
        clock: Arc<FakeClock>,
    }

    impl StatusReporter for ObservingReporter {
        fn report_stage(&self, _current: StageNumber, _total: StageNumber, _description: &str) {}
        fn report_complete(&self, _tool_key: LocalizationKey) {
            self.observed_clock_calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(self.clock.call_count());
        }
    }

    let observed_clock_calls = Arc::new(Mutex::new(Vec::new()));
    let clock = Arc::new(FakeClock::from_millis(&[0, 15]));
    let reporter_clock = Arc::clone(&clock);
    let reporter = VerboseTimingReporter::with_clock_and_writer(
        Box::new(ObservingReporter {
            observed_clock_calls: Arc::clone(&observed_clock_calls),
            clock: Arc::clone(&clock),
        }),
        test_prefs,
        Box::new(move || reporter_clock.now()),
        Vec::new(),
    );
    reporter.report_stage(
        StageNumber::new_unchecked(1),
        StageNumber::new_unchecked(6),
        "Reading manifest file",
    );
    reporter.report_complete(LocalizationKey::new(keys::STATUS_TOOL_GENERATE));

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
    let lines = render_summary_lines(test_prefs, state.completed_stages());
    let [header, stage_line, total_line] = lines.as_slice() else {
        panic!("expected 3 timing summary lines");
    };
    assert!(normalize_fluent_isolates(header).contains("Timing:"));
    assert!(normalize_fluent_isolates(header).contains("Stage timing summary:"));
    assert!(normalize_fluent_isolates(stage_line).contains("Stage 1/6: Reading manifest file"));
    assert!(normalize_fluent_isolates(stage_line).ends_with(": 15ms"));
    assert!(normalize_fluent_isolates(total_line).contains("Total pipeline time: 15ms"));
}

#[derive(Debug, Default)]
struct Counts {
    stages: usize,
    tasks: usize,
    completions: usize,
}

#[derive(Debug)]
struct CountingReporter {
    counts: Arc<Mutex<Counts>>,
}

impl StatusReporter for CountingReporter {
    fn report_stage(&self, _current: StageNumber, _total: StageNumber, _description: &str) {
        self.counts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stages += 1;
    }

    fn report_task_progress(&self, _current: u32, _total: u32, _description: &str) {
        self.counts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .tasks += 1;
    }

    fn report_complete(&self, _tool_key: LocalizationKey) {
        self.counts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .completions += 1;
    }
}

#[rstest]
fn verbose_timing_reporter_suppresses_progress_updates_after_complete(test_prefs: OutputPrefs) {
    let counts = Arc::new(Mutex::new(Counts::default()));
    let reporter = VerboseTimingReporter::with_clock_and_writer(
        Box::new(CountingReporter {
            counts: Arc::clone(&counts),
        }),
        test_prefs,
        Box::new(|| Duration::from_millis(50)),
        Vec::new(),
    );
    reporter.report_stage(
        StageNumber::new_unchecked(1),
        StageNumber::new_unchecked(6),
        "Reading manifest file",
    );
    reporter.report_task_progress(1, 2, "cc -c src/main.c");
    reporter.report_complete(LocalizationKey::new(keys::STATUS_TOOL_GENERATE));
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

#[rstest]
fn verbose_timing_reporter_writes_summary_to_injected_sink(en_localizer: EnLocalizer) {
    let _localizer = en_localizer;
    let clock = Arc::new(FakeClock::from_millis(&[0, 12, 23]));
    let injected_clock = Arc::clone(&clock);
    let output = Arc::new(Mutex::new(Vec::new()));
    let reporter = VerboseTimingReporter::with_clock_and_writer(
        Box::new(crate::status::SilentReporter),
        test_prefs(),
        Box::new(move || injected_clock.now()),
        SharedBufferWriter::new(Arc::clone(&output)),
    );
    reporter.report_stage(
        StageNumber::new_unchecked(1),
        StageNumber::new_unchecked(6),
        "Reading manifest file",
    );
    reporter.report_stage(
        StageNumber::new_unchecked(2),
        StageNumber::new_unchecked(6),
        "Parsing YAML document",
    );
    reporter.report_complete(LocalizationKey::new(keys::STATUS_TOOL_GENERATE));

    let captured_output = output
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let rendered = normalize_fluent_isolates(&String::from_utf8_lossy(&captured_output));
    assert!(rendered.contains("Stage timing summary:"));
    assert!(rendered.contains("Stage 1/6: Reading manifest file: 12ms"));
    assert!(rendered.contains("Stage 2/6: Parsing YAML document: 11ms"));
    assert!(rendered.contains("Total pipeline time: 23ms"));
}
