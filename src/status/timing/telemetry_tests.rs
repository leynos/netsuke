//! Telemetry tests for verbose timing-summary sink delivery.

use super::*;
use crate::output_prefs;
use crate::test_tracing_capture::with_test_subscriber;
use metrics_util::{
    CompositeKey, MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use rstest::rstest;
use std::io::{self, Write};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;
use test_support::{EnLocalizer, en_localizer};
use tracing_subscriber::filter::LevelFilter;

const TEST_TIMEOUT: Duration = Duration::from_secs(1);
type SnapshotEntry = (
    CompositeKey,
    Option<metrics::Unit>,
    Option<metrics::SharedString>,
    DebugValue,
);

/// Write every requested byte to a shared test buffer.
#[derive(Clone)]
struct BufferWriter(Arc<Mutex<Vec<u8>>>);

impl Write for BufferWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Fail every write with private data that telemetry must not reveal.
struct ErroringWriter;

impl Write for ErroringWriter {
    fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("private timing sink failure"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Block the first write until the test releases it, then record completion.
struct BlockingWriter {
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
    finished: mpsc::Sender<()>,
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
            self.finished
                .send(())
                .map_err(|error| io::Error::other(error.to_string()))?;
            self.has_blocked = true;
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Count forwarded completion events without retaining caller-controlled data.
struct CompletionReporter(Arc<Mutex<usize>>);

impl StatusReporter for CompletionReporter {
    fn report_stage(&self, _current: StageNumber, _total: StageNumber, _description: &str) {}

    fn report_complete(&self, _tool_key: LocalizationKey) {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
    }
}

/// Forwarded-event counts used to verify blocked-sink suppression.
#[derive(Default)]
struct ReporterCounts {
    stages: usize,
    progress: usize,
    completions: usize,
}

/// Record forwarded reporter events without retaining their descriptions.
struct RecordingReporter(Arc<Mutex<ReporterCounts>>);

impl StatusReporter for RecordingReporter {
    fn report_stage(&self, _current: StageNumber, _total: StageNumber, _description: &str) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stages += 1;
    }

    fn report_task_progress(&self, _current: u32, _total: u32, _description: &str) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .progress += 1;
    }

    fn report_complete(&self, _tool_key: LocalizationKey) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .completions += 1;
    }
}

/// Build a reporter with one timing stage and an injected sink.
fn reporter_with_writer<W: Write + Send>(writer: W) -> VerboseTimingReporter<W> {
    let reporter = VerboseTimingReporter::with_clock_and_writer(
        Box::new(crate::status::SilentReporter),
        output_prefs::resolve_with(None, |_| None),
        Box::new(|| Duration::from_millis(1)),
        writer,
    );
    reporter.report_stage(
        StageNumber::new_unchecked(1),
        StageNumber::new_unchecked(6),
        "telemetry stage",
    );
    reporter
}

/// Assert that `outcome` has one bounded timing-summary counter increment.
fn assert_sink_counter(snapshot: &[SnapshotEntry], outcome: &str) {
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Counter
                && entry.0.key().name() == TIMING_SUMMARY_SINK_WRITES_TOTAL
                && entry.0.key().labels().count() == 1
                && entry
                    .0
                    .key()
                    .labels()
                    .any(|label| label.key() == "outcome" && label.value() == outcome)
                && matches!(entry.3, DebugValue::Counter(1))
        }),
        "expected one timing-summary sink counter with outcome {outcome}: {snapshot:?}"
    );
}

/// Assert that one unlabelled timing-summary sink duration was recorded.
fn assert_sink_duration(snapshot: &[SnapshotEntry]) {
    assert!(
        snapshot.iter().any(|entry| {
            entry.0.kind() == MetricKind::Histogram
                && entry.0.key().name() == TIMING_SUMMARY_SINK_WRITE_DURATION
                && entry.0.key().labels().next().is_none()
                && matches!(entry.3, DebugValue::Histogram(ref samples) if samples.len() == 1)
        }),
        "expected one unlabelled timing-summary sink duration: {snapshot:?}"
    );
}

/// A successful sink records one successful delivery and no failure event.
#[rstest]
fn successful_timing_sink_records_bounded_telemetry(en_localizer: EnLocalizer) {
    let _localizer = en_localizer;
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let reporter = reporter_with_writer(BufferWriter(Arc::new(Mutex::new(Vec::new()))));
    let events = metrics::with_local_recorder(&recorder, || {
        with_test_subscriber(LevelFilter::DEBUG, |captured| {
            reporter.report_complete(LocalizationKey::new(keys::STATUS_TOOL_GENERATE));
            captured.snapshot()
        })
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_sink_counter(&snapshot, TIMING_SUMMARY_SINK_WRITE_SUCCESS);
    assert_sink_duration(&snapshot);
    assert!(
        events.is_empty(),
        "successful sink writes emit no failure event"
    );
}

/// A failed sink remains best-effort while emitting only bounded telemetry.
#[rstest]
fn erroring_timing_sink_records_bounded_failure_telemetry(en_localizer: EnLocalizer) {
    let _localizer = en_localizer;
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let completions = Arc::new(Mutex::new(0));
    let reporter = VerboseTimingReporter::with_clock_and_writer(
        Box::new(CompletionReporter(Arc::clone(&completions))),
        output_prefs::resolve_with(None, |_| None),
        Box::new(|| Duration::from_millis(1)),
        ErroringWriter,
    );
    reporter.report_stage(
        StageNumber::new_unchecked(1),
        StageNumber::new_unchecked(6),
        "caller-controlled telemetry stage",
    );
    let events = metrics::with_local_recorder(&recorder, || {
        with_test_subscriber(LevelFilter::DEBUG, |captured| {
            reporter.report_complete(LocalizationKey::new(keys::STATUS_TOOL_GENERATE));
            reporter.report_stage(
                StageNumber::new_unchecked(2),
                StageNumber::new_unchecked(6),
                "later caller-controlled stage",
            );
            reporter.report_complete(LocalizationKey::new(keys::STATUS_TOOL_GENERATE));
            captured.snapshot()
        })
    });

    assert_eq!(
        *completions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        1,
        "write failures must not repeat completion forwarding"
    );
    let snapshot = snapshotter.snapshot().into_vec();
    assert_sink_counter(&snapshot, TIMING_SUMMARY_SINK_WRITE_ERROR);
    assert_sink_duration(&snapshot);
    let [event] = events.as_slice() else {
        panic!("expected one bounded timing-summary write failure event: {events:?}");
    };
    assert!(event.contains("operation=\"timing_summary_sink_write\""));
    assert!(event.contains("outcome=\"write_error\""));
    assert!(event.contains("error_category=\"io\""));
    assert!(!event.contains("private timing sink failure"));
    assert!(!event.contains("caller-controlled telemetry stage"));
}

/// A blocked sink records its duration only after the synchronous write returns.
#[rstest]
fn blocking_timing_sink_records_duration_after_release(en_localizer: EnLocalizer) {
    let _localizer = en_localizer;
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let (entered_sender, entered_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let (finished_sender, finished_receiver) = mpsc::channel();
    let counts = Arc::new(Mutex::new(ReporterCounts::default()));
    let reporter = Arc::new(VerboseTimingReporter::with_clock_and_writer(
        Box::new(RecordingReporter(Arc::clone(&counts))),
        output_prefs::resolve_with(None, |_| None),
        Box::new(|| Duration::from_millis(1)),
        BlockingWriter {
            entered: entered_sender,
            release: release_receiver,
            finished: finished_sender,
            has_blocked: false,
        },
    ));
    reporter.report_stage(
        StageNumber::new_unchecked(1),
        StageNumber::new_unchecked(6),
        "first stage",
    );

    thread::scope(|scope| {
        let completion_reporter = Arc::clone(&reporter);
        let completion = scope.spawn(move || {
            metrics::with_local_recorder(&recorder, || {
                completion_reporter
                    .report_complete(LocalizationKey::new(keys::STATUS_TOOL_GENERATE));
            });
        });
        let entered = entered_receiver.recv_timeout(TEST_TIMEOUT);
        let before_release = snapshotter.snapshot().into_vec();
        reporter.report_stage(
            StageNumber::new_unchecked(2),
            StageNumber::new_unchecked(6),
            "suppressed stage",
        );
        reporter.report_task_progress(1, 1, "suppressed progress");
        reporter.report_complete(LocalizationKey::new(keys::STATUS_TOOL_GENERATE));
        let observed = {
            let observed = counts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (observed.stages, observed.progress, observed.completions)
        };

        let released = release_sender.send(());
        let joined = completion.join();
        let finished = finished_receiver.recv_timeout(TEST_TIMEOUT);
        entered.expect("the timing sink should block during its first write");
        released.expect("the blocked timing sink should be released");
        joined.expect("the completion caller should finish after release");
        finished.expect("the timing sink should finish before completion returns");
        assert!(
            before_release.is_empty(),
            "a blocked sink must not record duration before its write returns"
        );
        assert_eq!(observed, (1, 0, 1));
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_sink_counter(&snapshot, TIMING_SUMMARY_SINK_WRITE_SUCCESS);
    assert_sink_duration(&snapshot);
}
