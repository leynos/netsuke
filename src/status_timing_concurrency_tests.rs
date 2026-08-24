//! Concurrency tests for verbose timing summary sinks.

use super::*;
use crate::output_prefs;
use std::io::{self, Write};
use std::sync::{Arc, Barrier, Weak, mpsc};
use std::thread;
use std::time::Duration;
use test_support::fluent::normalize_fluent_isolates;

const TEST_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, Default)]
struct Counts {
    stages: usize,
    progress: usize,
    completions: usize,
}

#[derive(Debug)]
struct RecordingReporter {
    counts: Arc<Mutex<Counts>>,
}

impl StatusReporter for RecordingReporter {
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
            .progress += 1;
    }

    fn report_complete(&self, _tool_key: LocalizationKey) {
        self.counts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .completions += 1;
    }
}

struct BlockingWriter {
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
    has_blocked: bool,
}

impl Write for BlockingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !self.has_blocked {
            self.entered
                .send(())
                .map_err(|error| io::Error::other(error.to_string()))?;
            self.release
                .recv()
                .map_err(|error| io::Error::other(error.to_string()))?;
            self.has_blocked = true;
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn blocking_sink_prevents_later_status_forwarding_without_blocking_state_checks() {
    let counts = Arc::new(Mutex::new(Counts::default()));
    let (entered_sender, entered_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let reporter = Arc::new(VerboseTimingReporter::with_clock_and_writer(
        Box::new(RecordingReporter {
            counts: Arc::clone(&counts),
        }),
        output_prefs::resolve_with(None, |_| None),
        Box::new(|| Duration::from_millis(1)),
        BlockingWriter {
            entered: entered_sender,
            release: release_receiver,
            has_blocked: false,
        },
    ));
    reporter.report_stage(
        StageNumber::new_unchecked(1),
        StageNumber::new_unchecked(6),
        "first",
    );

    thread::scope(|scope| {
        let start = Arc::new(Barrier::new(2));
        let completion_reporter = Arc::clone(&reporter);
        let completion_start = Arc::clone(&start);
        let completion = scope.spawn(move || {
            completion_start.wait();
            completion_reporter.report_complete(LocalizationKey::new(keys::STATUS_TOOL_GENERATE));
        });
        start.wait();
        entered_receiver
            .recv_timeout(TEST_TIMEOUT)
            .expect("the writer should block after completion forwards");

        reporter.report_complete(LocalizationKey::new(keys::STATUS_TOOL_GENERATE));
        reporter.report_stage(
            StageNumber::new_unchecked(2),
            StageNumber::new_unchecked(6),
            "later",
        );
        reporter.report_task_progress(1, 1, "later task");
        let observed = counts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(observed.stages, 1);
        assert_eq!(observed.progress, 0);
        assert_eq!(observed.completions, 1);
        drop(observed);

        release_sender
            .send(())
            .expect("the blocked writer should be released");
        completion
            .join()
            .expect("the completion caller should finish after release");
    });
}

type ReentrantReporter = VerboseTimingReporter<ReentrantWriter>;

struct ReentrantWriter {
    reporter: Arc<Mutex<Option<Weak<ReentrantReporter>>>>,
    output: Arc<Mutex<Vec<u8>>>,
    reentered: mpsc::Sender<()>,
    has_reentered: bool,
}

impl Write for ReentrantWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !self.has_reentered {
            let reporter = self
                .reporter
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref()
                .and_then(Weak::upgrade)
                .ok_or_else(|| io::Error::other("the re-entrant reporter should still exist"))?;
            reporter.report_stage(
                StageNumber::new_unchecked(2),
                StageNumber::new_unchecked(6),
                "re-entry",
            );
            reporter.report_task_progress(1, 1, "re-entry");
            reporter.report_complete(LocalizationKey::new(keys::STATUS_TOOL_GENERATE));
            self.reentered
                .send(())
                .map_err(|error| io::Error::other(error.to_string()))?;
            self.has_reentered = true;
        }
        self.output
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn reentrant_sink_cannot_deadlock_or_duplicate_completion_output() {
    let counts = Arc::new(Mutex::new(Counts::default()));
    let output = Arc::new(Mutex::new(Vec::new()));
    let (reentered_sender, reentered_receiver) = mpsc::channel();
    let target = Arc::new(Mutex::new(None));
    let reporter = Arc::new(VerboseTimingReporter::with_clock_and_writer(
        Box::new(RecordingReporter {
            counts: Arc::clone(&counts),
        }),
        output_prefs::resolve_with(None, |_| None),
        Box::new(|| Duration::from_millis(1)),
        ReentrantWriter {
            reporter: Arc::clone(&target),
            output: Arc::clone(&output),
            reentered: reentered_sender,
            has_reentered: false,
        },
    ));
    *target
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::downgrade(&reporter));
    reporter.report_stage(
        StageNumber::new_unchecked(1),
        StageNumber::new_unchecked(6),
        "first",
    );

    thread::scope(|scope| {
        let start = Arc::new(Barrier::new(2));
        let completion_reporter = Arc::clone(&reporter);
        let completion_start = Arc::clone(&start);
        let completion = scope.spawn(move || {
            completion_start.wait();
            completion_reporter.report_complete(LocalizationKey::new(keys::STATUS_TOOL_GENERATE));
        });
        start.wait();
        reentered_receiver
            .recv_timeout(TEST_TIMEOUT)
            .expect("re-entry should complete without a deadlock");
        completion
            .join()
            .expect("the re-entrant completion call should return");
    });

    let observed = counts
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(observed.stages, 1);
    assert_eq!(observed.progress, 0);
    assert_eq!(observed.completions, 1);
    let captured_output = output
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let rendered = normalize_fluent_isolates(&String::from_utf8_lossy(&captured_output));
    assert_eq!(rendered.matches("Stage timing summary:").count(), 1);
}
