//! Fresh-process reproduction for stale manifest telemetry callsite interest.
//!
//! The ignored worker gates the first summary-event registration while a
//! second thread installs a TRACE subscriber, then emits the same production
//! callsites after T1 finishes. Running in a fresh test process ensures no
//! earlier test has registered those callsites.

mod driver;
mod gate;

use super::manifest_structure_telemetry::{
    record_manifest_structure, validate_manifest_structure_events,
};
use crate::ast::NetsukeManifest;
use anyhow::{Context, Result, anyhow, ensure};
use gate::{GATE_DEADLINE, GatedSubscriber, RegistrationGate};
use metrics_util::debugging::DebuggingRecorder;
use mockable::Env;
use std::{
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use tracing_subscriber::{filter::LevelFilter, prelude::*};

const MODE_ENV: &str = "NETSUKE_ISSUE_927_WORKER_MODE";
const THREAD_DEADLINE: Duration = Duration::from_secs(10);
const CHILD_DEADLINE: Duration = Duration::from_secs(20);
const EMPTY_MANIFEST_COUNTS: [(&str, usize); 6] = [
    ("variable_count", 0),
    ("macro_count", 0),
    ("rule_count", 0),
    ("action_count", 0),
    ("target_count", 0),
    ("default_count", 0),
];

/// Emit the ignored worker schedule selected through its child-only environment.
#[test]
#[ignore = "driven in a fresh process by the issue 927 parent test"]
fn interest_worker() -> Result<()> {
    let mode = mockable::DefaultEnv
        .os_string(MODE_ENV)
        .context("read the child-only schedule mode")?
        .into_string()
        .map_err(|mode| anyhow!("child-only schedule mode must be UTF-8: {mode:?}"))?;
    match mode.as_str() {
        "unmitigated" => run_unmitigated_worker(),
        "mitigated" => run_mitigated_worker(),
        _ => Err(anyhow!("unexpected issue 927 worker mode {mode:?}")),
    }
}

/// Prove the original stale-interest schedule still fails without anchors.
fn run_unmitigated_worker() -> Result<()> {
    let captured = run_unmitigated_threads(WorkerChannels::new(THREAD_DEADLINE))?;
    validate_unmitigated_capture(&captured)?;
    print_capture_markers(&captured, "installed", "zero-rejected")
}

/// Prove anchors prevent the stale-interest schedule from dropping the event.
fn run_mitigated_worker() -> Result<()> {
    test_support::tracing_capture::ensure_test_interest_anchors_registered();
    let captured = run_mitigated_threads(WorkerChannels::new(GATE_DEADLINE))?;
    validate_mitigated_capture(&captured)?;
    print_capture_markers(&captured, "deadline", "one-accepted")
}

/// Run both threads that reproduce the unmitigated dispatcher registration race.
fn run_unmitigated_threads(channels: WorkerChannels) -> Result<CaptureSnapshot> {
    let WorkerChannels {
        gate,
        paused_receiver,
        installed_sender,
        finished_sender,
        finished_receiver,
    } = channels;
    let t1_manifest = empty_manifest()?;
    let t2_manifest = empty_manifest()?;
    let debug_composition = tracing_subscriber::registry().with(LevelFilter::DEBUG);
    let gated_subscriber = GatedSubscriber::new(debug_composition, Arc::clone(&gate));
    let capture_thread = spawn_capture_thread(
        paused_receiver,
        installed_sender,
        finished_receiver,
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
        gate.failure().is_none(),
        "gate fixture failed: {:?}",
        gate.failure()
    );
    ensure!(
        gate.release_cause() == Some("installed"),
        "unmitigated T1 must release when T2 installs TRACE: {:?}",
        gate.release_cause()
    );
    Ok(captured)
}

/// Run the anchored schedule, where T2's dispatch registration waits for T1's read.
fn run_mitigated_threads(channels: WorkerChannels) -> Result<CaptureSnapshot> {
    let WorkerChannels {
        gate,
        paused_receiver,
        installed_sender,
        finished_sender,
        finished_receiver,
    } = channels;
    let t1_manifest = empty_manifest()?;
    let t2_manifest = empty_manifest()?;
    let debug_composition = tracing_subscriber::registry().with(LevelFilter::DEBUG);
    let gated_subscriber = GatedSubscriber::new(debug_composition, Arc::clone(&gate));
    let capture_thread = spawn_mitigated_capture_thread(
        paused_receiver,
        installed_sender,
        finished_receiver,
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
        gate.failure().is_none(),
        "gate fixture failed: {:?}",
        gate.failure()
    );
    ensure!(
        gate.release_cause() == Some("deadline"),
        "anchored T2 registration must wait for T1's read-side rebuild: {:?}; span fields: {:?}; events: {:?}; exact-cardinality oracle: {:?}",
        gate.release_cause(),
        captured.span_fields,
        captured.events,
        validate_manifest_structure_events(&captured.events, &EMPTY_MANIFEST_COUNTS)
    );
    Ok(captured)
}

/// Start T2 and capture the first event after T1's gated registration finishes.
fn spawn_capture_thread(
    paused_receiver: Receiver<()>,
    installed_sender: Sender<()>,
    finished_receiver: Receiver<()>,
    manifest: NetsukeManifest,
) -> JoinHandle<Result<CaptureSnapshot>> {
    thread::spawn(move || -> Result<CaptureSnapshot> {
        paused_receiver
            .recv_timeout(THREAD_DEADLINE)
            .context("wait for T1 to pause the summary registration")?;
        let (trace_subscriber, captured) =
            crate::test_tracing_capture::uninstalled_test_subscriber(LevelFilter::TRACE);
        tracing::subscriber::with_default(trace_subscriber, || -> Result<CaptureSnapshot> {
            installed_sender
                .send(())
                .context("signal that the TRACE dispatch is installed")?;
            finished_receiver
                .recv_timeout(THREAD_DEADLINE)
                .context("wait for T1 to finish its production emission")?;
            let recorder = DebuggingRecorder::new();
            metrics::with_local_recorder(&recorder, || {
                record_manifest_structure(&manifest);
            });
            Ok(CaptureSnapshot {
                events: captured.snapshot(),
                span_fields: captured.span_fields("runner.manifest.structure"),
            })
        })
    })
}

/// Start T2 with the shared capture helper and snapshot after T1 completes.
fn spawn_mitigated_capture_thread(
    paused_receiver: Receiver<()>,
    installed_sender: Sender<()>,
    finished_receiver: Receiver<()>,
    manifest: NetsukeManifest,
) -> JoinHandle<Result<CaptureSnapshot>> {
    thread::spawn(move || -> Result<CaptureSnapshot> {
        paused_receiver
            .recv_timeout(THREAD_DEADLINE)
            .context("wait for T1 to pause the summary registration")?;
        crate::test_tracing_capture::with_test_subscriber(LevelFilter::TRACE, |captured| {
            installed_sender
                .send(())
                .context("signal that the TRACE dispatch is installed")?;
            finished_receiver
                .recv_timeout(THREAD_DEADLINE)
                .context("wait for T1 to finish its production emission")?;
            let recorder = DebuggingRecorder::new();
            metrics::with_local_recorder(&recorder, || {
                record_manifest_structure(&manifest);
            });
            Ok(CaptureSnapshot {
                events: captured.snapshot(),
                span_fields: captured.span_fields("runner.manifest.structure"),
            })
        })
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

/// Require the unmitigated schedule to retain the span and lose the event.
fn validate_unmitigated_capture(captured: &CaptureSnapshot) -> Result<()> {
    ensure!(
        captured.span_fields.len() == 6,
        "the independent structural span callsite must be captured: {:?}",
        captured.span_fields
    );
    for (name, value) in [
        ("variable_count", 0),
        ("macro_count", 0),
        ("rule_count", 0),
        ("action_count", 0),
        ("target_count", 0),
        ("default_count", 0),
    ] {
        let expected = format!("{name}={value}");
        ensure!(
            captured
                .span_fields
                .iter()
                .filter(|field| **field == expected)
                .count()
                == 1,
            "the structural span must carry {expected:?}: {:?}",
            captured.span_fields
        );
    }
    ensure!(
        captured.events.is_empty(),
        "the unmitigated canary no longer loses the summary event: {:?}; upstream may have fixed the race and the test anchors may be removable",
        captured.events,
    );
    ensure!(
        validate_manifest_structure_events(&captured.events, &EMPTY_MANIFEST_COUNTS).is_err(),
        "the exact-cardinality oracle must reject the reproduced empty capture"
    );
    Ok(())
}

/// Require the anchored capture to satisfy the exact event oracle.
fn validate_mitigated_capture(captured: &CaptureSnapshot) -> Result<()> {
    ensure!(
        captured.span_fields.len() == 6,
        "the independent structural span callsite must be captured: {:?}",
        captured.span_fields
    );
    validate_manifest_structure_events(&captured.events, &EMPTY_MANIFEST_COUNTS).map_err(|error| {
        anyhow!(
            "anchored capture failed its exact-cardinality oracle: {error}; events: {:?}",
            captured.events
        )
    })
}

/// Print the release, span, event, and oracle markers required by the parent.
fn print_capture_markers(
    captured: &CaptureSnapshot,
    release_cause: &str,
    oracle: &str,
) -> Result<()> {
    write_marker("ISSUE927_GATE_PAUSED=true")?;
    write_marker(&format!("ISSUE927_RELEASE_CAUSE={release_cause}"))?;
    write_marker(&format!("ISSUE927_SPAN_FIELDS={:?}", captured.span_fields))?;
    write_marker(&format!("ISSUE927_EVENT_SNAPSHOT={:?}", captured.events))?;
    write_marker(&format!("ISSUE927_ORACLE={oracle}"))?;
    Ok(())
}

/// Own the phase channels consumed by the two worker threads.
struct WorkerChannels {
    gate: Arc<RegistrationGate>,
    paused_receiver: Receiver<()>,
    installed_sender: Sender<()>,
    finished_sender: Sender<()>,
    finished_receiver: Receiver<()>,
}

impl WorkerChannels {
    /// Build the gate and phase channels with the selected release deadline.
    fn new(gate_deadline: Duration) -> Self {
        let (paused_sender, paused_receiver) = mpsc::channel();
        let (installed_sender, installed_receiver) = mpsc::channel();
        let (finished_sender, finished_receiver) = mpsc::channel();
        Self {
            gate: Arc::new(RegistrationGate::new(
                paused_sender,
                installed_receiver,
                gate_deadline,
            )),
            paused_receiver,
            installed_sender,
            finished_sender,
            finished_receiver,
        }
    }
}

/// Parse an empty valid manifest before the race begins.
fn empty_manifest() -> Result<NetsukeManifest> {
    crate::manifest::from_str("netsuke_version: \"1.0.0\"\ntargets: []\n")
        .context("the empty reproduction manifest should parse")
}

/// Own the event and structural-span snapshot returned by T2.
struct CaptureSnapshot {
    events: Vec<String>,
    span_fields: Vec<String>,
}

/// Convert a worker panic into a fixture error rather than panicking the parent.
fn join_worker<T>(worker: JoinHandle<Result<T>>, name: &str) -> Result<T> {
    worker
        .join()
        .map_err(|_| anyhow!("{name} reproduction thread panicked"))?
}

/// Write one required phase marker to child stdout.
fn write_marker(marker: &str) -> std::io::Result<()> {
    use std::io::Write;

    let mut output = std::io::stdout().lock();
    writeln!(output, "{marker}")?;
    output.flush()
}
