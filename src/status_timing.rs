//! Verbose timing summary support for status reporting.

use super::{LocalizationKey, StageNumber, StatusReporter};
use crate::localization::{self, keys};
use std::io::{self, Write};
use std::sync::Mutex;
use std::time::{Duration, Instant};

type MonotonicClock = dyn Fn() -> Duration + Send + Sync;

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
struct TimingState {
    completed: bool,
    completed_stages: Vec<CompletedStage>,
    running: Option<RunningStage>,
}

impl TimingState {
    fn start_stage(&mut self, now: Duration, marker: StageMarker, description: &str) {
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

    fn completed_stages(&self) -> &[CompletedStage] {
        &self.completed_stages
    }

    fn finish_running(&mut self, now: Duration) {
        let Some(running) = self.running.take() else {
            return;
        };
        let elapsed = now.saturating_sub(running.started_at);
        self.completed_stages.push(CompletedStage {
            marker: running.marker,
            description: running.description,
            elapsed,
        });
    }
}

/// Status reporter wrapper that emits per-stage timings on successful
/// completion.
pub struct VerboseTimingReporter {
    inner: Box<dyn StatusReporter>,
    clock: Box<MonotonicClock>,
    state: Mutex<TimingState>,
}

impl VerboseTimingReporter {
    /// Wrap an existing reporter with verbose timing summary support.
    #[must_use]
    pub fn new(inner: Box<dyn StatusReporter>) -> Self {
        let start = Instant::now();
        Self::with_clock(inner, Box::new(move || start.elapsed()))
    }

    fn with_clock(inner: Box<dyn StatusReporter>, clock: Box<MonotonicClock>) -> Self {
        Self {
            inner,
            clock,
            state: Mutex::new(TimingState::default()),
        }
    }
}

impl StatusReporter for VerboseTimingReporter {
    fn report_stage(&self, current: StageNumber, total: StageNumber, description: &str) {
        let should_forward = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.completed {
                false
            } else {
                state.start_stage((self.clock)(), StageMarker { current, total }, description);
                true
            }
        };
        if should_forward {
            self.inner.report_stage(current, total, description);
        }
    }

    fn report_task_progress(&self, current: u32, total: u32, description: &str) {
        let should_forward = {
            let state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            !state.completed
        };
        if should_forward {
            self.inner.report_task_progress(current, total, description);
        }
    }

    fn report_complete(&self, tool_key: LocalizationKey) {
        let lines = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.completed {
                Vec::new()
            } else {
                state.completed = true;
                state.finish((self.clock)());
                render_summary_lines(state.completed_stages())
            }
        };

        self.inner.report_complete(tool_key);

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
    let seconds = duration.as_secs();
    if seconds > 0 {
        let milliseconds = duration.subsec_millis();
        if milliseconds == 0 {
            return format!("{seconds}s");
        }
        return format!("{seconds}.{milliseconds:03}s");
    }
    if duration.as_millis() > 0 {
        return format!("{}ms", duration.as_millis());
    }
    if duration.as_micros() > 0 {
        return format!("{}us", duration.as_micros());
    }
    format!("{}ns", duration.as_nanos())
}

#[path = "status_timing_tests.rs"]
#[cfg(test)]
mod tests;
