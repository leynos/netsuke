//! Coordinate three gated capture schedules used by the fresh-process test.
//!
//! These helpers stay private to the issue 927 reproduction. The three modes
//! share channel orchestration and event snapshots while retaining distinct
//! subscriber installation boundaries.

mod capture;

use super::super::manifest_structure_telemetry::record_manifest_structure;
use super::gate::{GATE_DEADLINE, GatedSubscriber, RegistrationGate, RegistrationGateChannels};
use super::{EMPTY_MANIFEST_COUNTS, validate_manifest_structure_events};
use crate::ast::NetsukeManifest;
use anyhow::{Context, Result, anyhow, ensure};
use capture::spawn_capture_thread;
use metrics_util::debugging::DebuggingRecorder;
use std::{
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use tracing_subscriber::{filter::LevelFilter, prelude::*};

const THREAD_DEADLINE: Duration = Duration::from_secs(10);

/// Select the T2 capture boundary and expected registration release cause.
#[derive(Clone, Copy)]
pub(super) enum CaptureMode {
    /// Capture with the uninstalled constructor and release when T2 installs.
    Unmitigated,
    /// Capture with the shared helper and release after the fixed deadline.
    Mitigated,
    /// Capture through `test_support::with_test_subscriber` and release by deadline.
    PublicHelper,
}

/// Own the event and structural-span snapshot returned by T2.
pub(super) struct CaptureSnapshot {
    pub(super) events: Vec<String>,
    pub(super) span_fields: Vec<String>,
}

/// Record when T2 reached the capture layer's real summary registration callback.
pub(super) struct RegistrationAttemptTiming {
    pub(super) dispatch_started_before_release: bool,
    pub(super) summary_callback_before_release: bool,
    pub(super) summary_callback_after_release: bool,
}

/// Pair T2's capture with the callback timing observed by T1's gate.
pub(super) struct WorkerSnapshot {
    pub(super) capture: CaptureSnapshot,
    pub(super) registration_timing: RegistrationAttemptTiming,
}

/// Keep the gate and both worker handles together until the schedule completes.
struct RunningWorkers {
    gate: Arc<RegistrationGate>,
    emission_thread: JoinHandle<Result<()>>,
    capture_thread: JoinHandle<Result<CaptureSnapshot>>,
}

/// Run the shared thread schedule with the selected T2 capture boundary.
pub(super) fn run_worker_threads(capture_mode: CaptureMode) -> Result<WorkerSnapshot> {
    finish_worker_threads(capture_mode, start_worker_threads(capture_mode)?)
}

/// Start both workers after constructing their inputs and shared gate.
fn start_worker_threads(capture_mode: CaptureMode) -> Result<RunningWorkers> {
    let channels = WorkerChannels::new(capture_mode);
    let WorkerChannels {
        gate,
        paused_receiver,
        registration_started_sender,
        summary_callback_sender,
        installed_sender,
        finished_sender,
        finished_receiver,
    } = channels;
    let t1_manifest = empty_manifest()?;
    let t2_manifest = empty_manifest()?;
    let debug_composition = tracing_subscriber::registry().with(LevelFilter::DEBUG);
    let gated_subscriber = GatedSubscriber::new(debug_composition, Arc::clone(&gate));
    let capture_thread = spawn_capture_thread(
        capture_mode,
        CaptureThreadChannels {
            paused_receiver,
            registration_started_sender,
            summary_callback_sender,
            installed_sender,
            finished_receiver,
        },
        t2_manifest,
    );
    let emission_thread = spawn_emission_thread(gated_subscriber, finished_sender, t1_manifest);

    Ok(RunningWorkers {
        gate,
        emission_thread,
        capture_thread,
    })
}

/// Join both workers and validate their observed registration phases.
fn finish_worker_threads(
    capture_mode: CaptureMode,
    workers: RunningWorkers,
) -> Result<WorkerSnapshot> {
    join_worker(workers.emission_thread, "T1")?;
    let captured = join_worker(workers.capture_thread, "T2")?;
    let registration_timing = validate_registration_timing(&workers.gate, capture_mode)?;
    validate_gate_outcome(&workers.gate, capture_mode, &captured)?;
    Ok(WorkerSnapshot {
        capture: captured,
        registration_timing,
    })
}

/// Require each callback to occur on the side of release expected by its mode.
fn validate_registration_timing(
    gate: &RegistrationGate,
    capture_mode: CaptureMode,
) -> Result<RegistrationAttemptTiming> {
    ensure!(
        gate.was_paused(),
        "T1 did not reach the summary-event registration gate"
    );
    let timing = RegistrationAttemptTiming {
        dispatch_started_before_release: gate.registration_started_before_release(),
        summary_callback_before_release: gate.summary_callback_before_release(),
        summary_callback_after_release: gate.take_summary_callback_after_release(),
    };
    ensure!(
        timing.dispatch_started_before_release,
        "T2 did not enter Dispatch::new before T1 released its registration gate"
    );
    ensure!(
        timing.summary_callback_before_release == matches!(capture_mode, CaptureMode::Unmitigated),
        "T2 summary callback had the wrong timing relative to T1's gate: before release={}, after release={}",
        timing.summary_callback_before_release,
        timing.summary_callback_after_release
    );
    ensure!(
        timing.summary_callback_after_release
            == matches!(
                capture_mode,
                CaptureMode::Mitigated | CaptureMode::PublicHelper
            ),
        "T2 summary callback had the wrong timing relative to T1's gate: before release={}, after release={}",
        timing.summary_callback_before_release,
        timing.summary_callback_after_release
    );
    Ok(timing)
}

/// Require the gate to complete without fixture errors and with the expected cause.
fn validate_gate_outcome(
    gate: &RegistrationGate,
    capture_mode: CaptureMode,
    captured: &CaptureSnapshot,
) -> Result<()> {
    ensure!(
        gate.failure().is_none(),
        "gate fixture failed: {:?}",
        gate.failure()
    );
    ensure!(
        gate.release_cause() == Some(capture_mode.expected_release()),
        "{}: {:?}; span fields: {:?}; events: {:?}; exact-cardinality oracle: {:?}",
        capture_mode.release_requirement(),
        gate.release_cause(),
        captured.span_fields,
        captured.events,
        validate_manifest_structure_events(&captured.events, &EMPTY_MANIFEST_COUNTS)
    );
    Ok(())
}

impl CaptureMode {
    /// Return the expected event-registration release marker for this mode.
    fn expected_release(self) -> &'static str {
        match self {
            Self::Unmitigated => "installed",
            Self::Mitigated | Self::PublicHelper => "deadline",
        }
    }

    /// Explain the selected mode's expected registration release cause.
    fn release_requirement(self) -> &'static str {
        match self {
            Self::Unmitigated => "unmitigated T1 must release when T2 installs TRACE",
            Self::Mitigated | Self::PublicHelper => {
                "anchored T2 registration must wait for T1's read-side rebuild"
            }
        }
    }
}

/// Start T1 with the DEBUG composition wrapped by the single-callsite gate.
fn spawn_emission_thread(
    subscriber: GatedSubscriber<impl tracing::Subscriber + Send + Sync>,
    finished_sender: Sender<()>,
    manifest: NetsukeManifest,
) -> JoinHandle<Result<()>> {
    thread::spawn(move || -> Result<()> {
        tracing::subscriber::with_default(subscriber, || {
            let recorder = DebuggingRecorder::new();
            metrics::with_local_recorder(&recorder, || {
                record_manifest_structure(&manifest);
            });
        });
        finished_sender
            .send(())
            .context("signal that T1 finished its production emission")?;
        Ok(())
    })
}

/// Parse an empty valid manifest before the race begins.
fn empty_manifest() -> Result<NetsukeManifest> {
    crate::manifest::from_str("netsuke_version: \"1.0.0\"\ntargets: []\n")
        .context("the empty reproduction manifest should parse")
}

/// Own the phase channels consumed by the two worker threads.
struct WorkerChannels {
    gate: Arc<RegistrationGate>,
    paused_receiver: Receiver<()>,
    registration_started_sender: Sender<()>,
    summary_callback_sender: Sender<()>,
    installed_sender: Sender<()>,
    finished_sender: Sender<()>,
    finished_receiver: Receiver<()>,
}

/// Own the phase channels consumed by T2's capture thread.
struct CaptureThreadChannels {
    paused_receiver: Receiver<()>,
    registration_started_sender: Sender<()>,
    summary_callback_sender: Sender<()>,
    installed_sender: Sender<()>,
    finished_receiver: Receiver<()>,
}

/// Own the channels used by T2's selected capture mode.
struct CaptureModeChannels {
    /// Signal that T2 entered the dispatch-registration boundary.
    registration_started_sender: Sender<()>,
    /// Signal that T2 installed its tracing dispatch.
    installed_sender: Sender<()>,
    /// Wait for T1's production emission to complete.
    finished_receiver: Receiver<()>,
}

impl WorkerChannels {
    /// Build channels and select the schedule's fixed registration deadline.
    fn new(capture_mode: CaptureMode) -> Self {
        let gate_deadline = match capture_mode {
            CaptureMode::Unmitigated => THREAD_DEADLINE,
            CaptureMode::Mitigated | CaptureMode::PublicHelper => GATE_DEADLINE,
        };
        let (paused_sender, paused_receiver) = mpsc::channel();
        let (registration_started_sender, registration_started_receiver) = mpsc::channel();
        let (summary_callback_sender, summary_callback_receiver) = mpsc::channel();
        let (installed_sender, installed_receiver) = mpsc::channel();
        let (finished_sender, finished_receiver) = mpsc::channel();
        Self {
            gate: Arc::new(RegistrationGate::new(
                RegistrationGateChannels {
                    paused_sender,
                    registration_started_receiver,
                    summary_callback_receiver,
                    installed_receiver,
                },
                gate_deadline,
                THREAD_DEADLINE,
            )),
            paused_receiver,
            registration_started_sender,
            summary_callback_sender,
            installed_sender,
            finished_sender,
            finished_receiver,
        }
    }
}

/// Convert a worker panic into a fixture error rather than panicking the parent.
fn join_worker<T>(worker: JoinHandle<Result<T>>, name: &str) -> Result<T> {
    worker
        .join()
        .map_err(|_| anyhow!("{name} reproduction thread panicked"))?
}
