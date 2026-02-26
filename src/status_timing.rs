//! Verbose timing summary support for status reporting.

use super::{LocalizationKey, StageNumber, StatusReporter};
use crate::localization::{self, keys};
use std::io::{self, Write};
use std::sync::Mutex;
use std::time::{Duration, Instant};

trait MonotonicClock: Send + Sync {
    fn now(&self) -> Duration;
}

#[derive(Debug)]
struct SystemMonotonicClock {
    start: Instant,
}

impl Default for SystemMonotonicClock {
    fn default() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl MonotonicClock for SystemMonotonicClock {
    fn now(&self) -> Duration {
        self.start.elapsed()
    }
}

#[derive(Debug, Copy, Clone)]
struct StageMarker {
    current: StageNumber,
    total: StageNumber,
}

#[derive(Debug, Clone)]
struct RunningStage {
    marker: StageMarker,
    description: String,
    started_at: Duration,
}

#[derive(Debug, Clone)]
struct CompletedStage {
    marker: StageMarker,
    description: String,
    elapsed: Duration,
}

#[derive(Debug, Default)]
struct StageTimingRecorder {
    completed: Vec<CompletedStage>,
    running: Option<RunningStage>,
}

impl StageTimingRecorder {
    fn record_stage(&mut self, now: Duration, marker: StageMarker, description: &str) {
        self.finish_running(now);
        self.running = Some(RunningStage {
            marker,
            description: description.to_owned(),
            started_at: now,
        });
    }

    fn finish(&mut self, now: Duration) {
        self.finish_running(now);
    }

    fn completed(&self) -> &[CompletedStage] {
        &self.completed
    }

    fn finish_running(&mut self, now: Duration) {
        let Some(running) = self.running.take() else {
            return;
        };
        let elapsed = now.saturating_sub(running.started_at);
        self.completed.push(CompletedStage {
            marker: running.marker,
            description: running.description,
            elapsed,
        });
    }
}

#[derive(Debug, Default)]
struct TimingState {
    completed: bool,
    recorder: StageTimingRecorder,
}

/// Status reporter wrapper that emits per-stage timings on successful
/// completion.
pub struct VerboseTimingReporter {
    inner: Box<dyn StatusReporter>,
    clock: Box<dyn MonotonicClock>,
    state: Mutex<TimingState>,
}

impl VerboseTimingReporter {
    /// Wrap an existing reporter with verbose timing summary support.
    #[must_use]
    pub fn new(inner: Box<dyn StatusReporter>) -> Self {
        Self::with_clock(inner, Box::new(SystemMonotonicClock::default()))
    }

    fn with_clock(inner: Box<dyn StatusReporter>, clock: Box<dyn MonotonicClock>) -> Self {
        Self {
            inner,
            clock,
            state: Mutex::new(TimingState::default()),
        }
    }
}

impl StatusReporter for VerboseTimingReporter {
    fn report_stage(&self, current: StageNumber, total: StageNumber, description: &str) {
        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !state.completed {
                state.recorder.record_stage(
                    self.clock.now(),
                    StageMarker { current, total },
                    description,
                );
            }
        }
        self.inner.report_stage(current, total, description);
    }

    fn report_task_progress(&self, current: u32, total: u32, description: &str) {
        self.inner.report_task_progress(current, total, description);
    }

    fn report_complete(&self, tool_key: LocalizationKey) {
        self.inner.report_complete(tool_key);

        let lines = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.completed {
                Vec::new()
            } else {
                state.completed = true;
                state.recorder.finish(self.clock.now());
                render_summary_lines(state.recorder.completed())
            }
        };

        for line in lines {
            drop(writeln!(io::stderr(), "{line}"));
        }
    }
}

fn render_summary_lines(entries: &[CompletedStage]) -> Vec<String> {
    if entries.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::with_capacity(entries.len() + 2);
    lines.push(localization::message(keys::STATUS_TIMING_SUMMARY_HEADER).to_string());

    for entry in entries {
        let label = localization::message(keys::STATUS_STAGE_LABEL)
            .with_arg("current", entry.marker.current.get().to_string())
            .with_arg("total", entry.marker.total.get().to_string())
            .with_arg("description", &entry.description)
            .to_string();
        let line = localization::message(keys::STATUS_TIMING_STAGE_LINE)
            .with_arg("label", &label)
            .with_arg("duration", format_duration(entry.elapsed))
            .to_string();
        lines.push(line);
    }

    let total = entries.iter().fold(Duration::ZERO, |acc, entry| {
        acc.saturating_add(entry.elapsed)
    });
    lines.push(
        localization::message(keys::STATUS_TIMING_TOTAL_LINE)
            .with_arg("duration", format_duration(total))
            .to_string(),
    );

    lines
}

fn format_duration(duration: Duration) -> String {
    if duration.as_secs() > 0 {
        return format!("{}s", duration.as_secs());
    }
    if duration.as_millis() > 0 {
        return format!("{}ms", duration.as_millis());
    }
    if duration.as_micros() > 0 {
        return format!("{}us", duration.as_micros());
    }
    format!("{}ns", duration.as_nanos())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::collections::VecDeque;

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
            }
        }
    }

    impl MonotonicClock for FakeClock {
        fn now(&self) -> Duration {
            let mut values = self
                .values
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            values.pop_front().unwrap_or(self.fallback)
        }
    }

    #[rstest]
    fn timing_recorder_renders_happy_path_summary() {
        let total = StageNumber::new_unchecked(6);
        let mut recorder = StageTimingRecorder::default();
        recorder.record_stage(
            Duration::from_millis(0),
            StageMarker {
                current: StageNumber::new_unchecked(1),
                total,
            },
            "Reading manifest file",
        );
        recorder.record_stage(
            Duration::from_millis(12),
            StageMarker {
                current: StageNumber::new_unchecked(2),
                total,
            },
            "Parsing YAML document",
        );
        recorder.record_stage(
            Duration::from_millis(16),
            StageMarker {
                current: StageNumber::new_unchecked(3),
                total,
            },
            "Expanding template directives",
        );
        recorder.finish(Duration::from_millis(23));

        let lines = render_summary_lines(recorder.completed());
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
        let mut recorder = StageTimingRecorder::default();
        recorder.record_stage(
            Duration::from_millis(0),
            StageMarker {
                current: StageNumber::new_unchecked(1),
                total,
            },
            "Reading manifest file",
        );

        let lines = render_summary_lines(recorder.completed());
        assert!(lines.is_empty());
    }

    #[rstest]
    #[case(Duration::from_nanos(7), "7ns")]
    #[case(Duration::from_micros(18), "18us")]
    #[case(Duration::from_millis(22), "22ms")]
    #[case(Duration::from_secs(3), "3s")]
    fn duration_formatting_uses_expected_units(#[case] duration: Duration, #[case] expected: &str) {
        assert_eq!(format_duration(duration), expected);
    }

    #[rstest]
    fn verbose_timing_reporter_finalizes_current_stage_on_complete() {
        struct NoopReporter;
        impl StatusReporter for NoopReporter {
            fn report_stage(&self, _current: StageNumber, _total: StageNumber, _description: &str) {
            }
            fn report_complete(&self, _tool_key: LocalizationKey) {}
        }

        let reporter = VerboseTimingReporter::with_clock(
            Box::new(NoopReporter),
            Box::new(FakeClock::from_millis(&[0, 15])),
        );
        reporter.report_stage(
            StageNumber::new_unchecked(1),
            StageNumber::new_unchecked(6),
            "Reading manifest file",
        );
        reporter.report_complete(LocalizationKey::new(keys::STATUS_TOOL_MANIFEST));
    }
}
