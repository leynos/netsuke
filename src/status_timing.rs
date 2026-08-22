//! Verbose timing summary support for status reporting.

use super::{LocalizationKey, StageNumber, StatusReporter};
use crate::localization::{self, keys};
use crate::output_prefs::OutputPrefs;
use std::io::{self, Write};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Monotonic clock abstraction returning an elapsed duration on demand.
type MonotonicClock = dyn Fn() -> Duration + Send + Sync;

/// Identify a stage by its position within the overall stage count.
#[derive(Debug, Copy, Clone)]
struct StageMarker {
    /// One-based index of the stage being reported.
    current: StageNumber,
    /// Total number of stages in the run.
    total: StageNumber,
}

/// Describe a stage that is currently in progress.
#[derive(Debug, Clone)]
struct RunningStage {
    /// Marker identifying the running stage.
    marker: StageMarker,
    /// Human-readable description of the running stage.
    description: String,
    /// Clock value recorded when the stage began.
    started_at: Duration,
}

/// Record a stage that has finished and the time it took.
#[derive(Debug, Clone)]
struct CompletedStage {
    /// Marker identifying the finished stage.
    marker: StageMarker,
    /// Human-readable description of the finished stage.
    description: String,
    /// Duration the stage ran before completion.
    elapsed: Duration,
}

/// Track the timing summary being accumulated for one run.
#[derive(Debug, Default)]
struct TimingState {
    /// Whether completion has already been recorded for this run.
    completed: bool,
    /// Stages finished so far, in completion order.
    completed_stages: Vec<CompletedStage>,
    /// Stage currently in progress, if any.
    running: Option<RunningStage>,
}

impl TimingState {
    /// Finish the stage currently running, if any, then record a new running
    /// stage.
    fn start_stage(&mut self, now: Duration, marker: StageMarker, description: &str) {
        self.finish_running(now);
        self.running = Some(RunningStage {
            marker,
            description: description.to_owned(),
            started_at: now,
        });
    }

    /// Finish the stage currently running, if any.
    fn finish(&mut self, now: Duration) {
        self.finish_running(now);
    }

    /// Return the stages finished so far, in completion order.
    fn completed_stages(&self) -> &[CompletedStage] {
        &self.completed_stages
    }

    /// Move the running stage, if any, into the completed list.
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
///
/// The writer defaults to [`io::Stderr`]; tests can supply a `Vec<u8>`
/// via the test-only writer-and-clock constructor for output capture.
pub struct VerboseTimingReporter<W: Write + Send = io::Stderr> {
    /// Reporter receiving forwarded status events.
    inner: Box<dyn StatusReporter>,
    /// Output preferences controlling summary formatting.
    prefs: OutputPrefs,
    /// Clock used to timestamp stage transitions.
    clock: Box<MonotonicClock>,
    /// Timing state shared across reporting threads.
    state: Mutex<TimingState>,
    /// Sink receiving the rendered timing summary.
    writer: Mutex<W>,
}

impl VerboseTimingReporter {
    /// Wrap an existing reporter with verbose timing summary support.
    #[must_use]
    pub fn new(inner: Box<dyn StatusReporter>, prefs: OutputPrefs) -> Self {
        let start = Instant::now();
        Self::with_clock(inner, prefs, Box::new(move || start.elapsed()))
    }

    /// Wrap an existing reporter with verbose timing summary support and an
    /// injected monotonic clock.
    fn with_clock(
        inner: Box<dyn StatusReporter>,
        prefs: OutputPrefs,
        clock: Box<MonotonicClock>,
    ) -> Self {
        Self::with_clock_and_writer(inner, prefs, clock, io::stderr())
    }
}

impl<W: Write + Send> VerboseTimingReporter<W> {
    /// Wrap an existing reporter with verbose timing summary support, an
    /// injected monotonic clock, and an injected timing summary sink.
    fn with_clock_and_writer(
        inner: Box<dyn StatusReporter>,
        prefs: OutputPrefs,
        clock: Box<MonotonicClock>,
        writer: W,
    ) -> Self {
        Self {
            inner,
            prefs,
            clock,
            state: Mutex::new(TimingState::default()),
            writer: Mutex::new(writer),
        }
    }
}

impl<W: Write + Send> StatusReporter for VerboseTimingReporter<W> {
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
                render_summary_lines(self.prefs, state.completed_stages())
            }
        };

        self.inner.report_complete(tool_key);

        let mut writer = self
            .writer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for line in lines {
            drop(writeln!(writer, "{line}"));
        }
    }
}
/// Render the timing summary header, per-stage lines, and total duration.
fn render_summary_lines(prefs: OutputPrefs, entries: &[CompletedStage]) -> Vec<String> {
    if entries.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::with_capacity(entries.len() + 2);
    let prefix = prefs.timing_prefix();
    let header = localization::message(keys::STATUS_TIMING_SUMMARY_HEADER);
    lines.push(format!("{prefix} {header}"));

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
        lines.push(format!("{}{line}", prefs.timing_indent()));
    }

    let total = entries.iter().fold(Duration::ZERO, |acc, entry| {
        acc.saturating_add(entry.elapsed)
    });
    let total_line = localization::message(keys::STATUS_TIMING_TOTAL_LINE)
        .with_arg("duration", format_duration(total))
        .to_string();
    lines.push(format!("{}{total_line}", prefs.timing_indent()));

    lines
}

/// Format a duration with the unit (ns, us, ms, or s) chosen for readability.
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
