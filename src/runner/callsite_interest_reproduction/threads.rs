//! Coordinate the two gated capture schedules used by the fresh-process test.
//!
//! These helpers stay private to the issue 927 reproduction. The two modes
//! share channel orchestration and event snapshots while retaining distinct
//! subscriber installation boundaries.

use super::super::manifest_structure_telemetry::record_manifest_structure;
use super::gate::{GATE_DEADLINE, GatedSubscriber, RegistrationGate, RegistrationGateOptions};
use super::{EMPTY_MANIFEST_COUNTS, validate_manifest_structure_events};
use crate::ast::NetsukeManifest;
use crate::test_tracing_capture::CapturedEvents;
use anyhow::{Context, Result, anyhow, ensure};
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
}

/// Own the event and structural-span snapshot returned by T2.
pub(super) struct CaptureSnapshot {
    pub(super) events: Vec<String>,
    pub(super) span_fields: Vec<String>,
}

/// Run the shared thread schedule with the selected T2 capture boundary.
pub(super) fn run_worker_threads(capture_mode: CaptureMode) -> Result<CaptureSnapshot> {
    let channels = WorkerChannels::new(capture_mode);
    let WorkerChannels {
        gate,
        paused_receiver,
        registration_attempt_sender,
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
            registration_attempt_sender,
            installed_sender,
            finished_receiver,
        },
        t2_manifest,
    );
    let emission_thread = spawn_emission_thread(gated_subscriber, finished_sender, t1_manifest);

    join_worker(emission_thread, "T1")?;
    let captured = join_worker(capture_thread, "T2")?;
    ensure!(
        gate.was_paused(),
        "T1 did not reach the summary-event registration gate"
    );
    ensure!(
        gate.was_attempted(),
        "T2 did not reach the capture layer's summary registration callback"
    );
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
    Ok(captured)
}

impl CaptureMode {
    /// Return the expected event-registration release marker for this mode.
    fn expected_release(self) -> &'static str {
        match self {
            Self::Unmitigated => "installed",
            Self::Mitigated => "deadline",
        }
    }

    /// Explain the selected mode's expected registration release cause.
    fn release_requirement(self) -> &'static str {
        match self {
            Self::Unmitigated => "unmitigated T1 must release when T2 installs TRACE",
            Self::Mitigated => "anchored T2 registration must wait for T1's read-side rebuild",
        }
    }
}

/// Start T2 with the selected subscriber installation boundary.
fn spawn_capture_thread(
    capture_mode: CaptureMode,
    channels: CaptureThreadChannels,
    manifest: NetsukeManifest,
) -> JoinHandle<Result<CaptureSnapshot>> {
    let CaptureThreadChannels {
        paused_receiver,
        registration_attempt_sender,
        installed_sender,
        finished_receiver,
    } = channels;
    thread::spawn(move || {
        paused_receiver
            .recv_timeout(THREAD_DEADLINE)
            .context("wait for T1 to pause the summary registration")?;
        crate::test_tracing_capture::with_manifest_summary_registration_signal(
            registration_attempt_sender,
            || match capture_mode {
                CaptureMode::Unmitigated => {
                    let (trace_subscriber, captured) =
                        crate::test_tracing_capture::uninstalled_test_subscriber(
                            LevelFilter::TRACE,
                        );
                    tracing::subscriber::with_default(trace_subscriber, || {
                        capture_manifest_after_install(
                            &installed_sender,
                            &finished_receiver,
                            &manifest,
                            &captured,
                        )
                    })
                }
                CaptureMode::Mitigated => crate::test_tracing_capture::with_test_subscriber(
                    LevelFilter::TRACE,
                    |captured| {
                        capture_manifest_after_install(
                            &installed_sender,
                            &finished_receiver,
                            &manifest,
                            &captured,
                        )
                    },
                ),
            },
        )
    })
}

/// Signal dispatch installation and snapshot the production emission after T1 ends.
fn capture_manifest_after_install(
    installed_sender: &Sender<()>,
    finished_receiver: &Receiver<()>,
    manifest: &NetsukeManifest,
    captured: &CapturedEvents,
) -> Result<CaptureSnapshot> {
    installed_sender
        .send(())
        .context("signal that the TRACE dispatch is installed")?;
    finished_receiver
        .recv_timeout(THREAD_DEADLINE)
        .context("wait for T1 to finish its production emission")?;
    let recorder = DebuggingRecorder::new();
    metrics::with_local_recorder(&recorder, || {
        record_manifest_structure(manifest);
    });
    Ok(CaptureSnapshot {
        events: captured.snapshot(),
        span_fields: captured.span_fields("runner.manifest.structure"),
    })
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
    registration_attempt_sender: Sender<()>,
    installed_sender: Sender<()>,
    finished_sender: Sender<()>,
    finished_receiver: Receiver<()>,
}

/// Own the phase channels consumed by T2's capture thread.
struct CaptureThreadChannels {
    paused_receiver: Receiver<()>,
    registration_attempt_sender: Sender<()>,
    installed_sender: Sender<()>,
    finished_receiver: Receiver<()>,
}

impl WorkerChannels {
    /// Build channels and select the schedule's fixed registration deadline.
    fn new(capture_mode: CaptureMode) -> Self {
        let gate_deadline = match capture_mode {
            CaptureMode::Unmitigated => THREAD_DEADLINE,
            CaptureMode::Mitigated => GATE_DEADLINE,
        };
        let attempt_timeout_is_expected = matches!(capture_mode, CaptureMode::Mitigated);
        let (paused_sender, paused_receiver) = mpsc::channel();
        let (registration_attempt_sender, registration_attempt_receiver) = mpsc::channel();
        let (installed_sender, installed_receiver) = mpsc::channel();
        let (finished_sender, finished_receiver) = mpsc::channel();
        Self {
            gate: Arc::new(RegistrationGate::new(
                paused_sender,
                registration_attempt_receiver,
                installed_receiver,
                RegistrationGateOptions {
                    deadline: gate_deadline,
                    attempt_timeout_is_expected,
                },
            )),
            paused_receiver,
            registration_attempt_sender,
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
