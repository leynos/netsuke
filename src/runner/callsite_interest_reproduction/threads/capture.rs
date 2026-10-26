//! Run T2 through the reproduction's subscriber installation boundaries.

use super::super::super::manifest_structure_telemetry::record_manifest_structure;
use super::{
    CaptureMode, CaptureModeChannels, CaptureSnapshot, CaptureThreadChannels, THREAD_DEADLINE,
};
use crate::ast::NetsukeManifest;
use anyhow::{Context, Result, ensure};
use metrics_util::debugging::DebuggingRecorder;
use std::{
    cell::Cell,
    rc::Rc,
    sync::mpsc::{Receiver, Sender},
    thread::{self, JoinHandle},
};
use tracing_subscriber::filter::LevelFilter;

/// Start T2 with the selected subscriber installation boundary.
pub(super) fn spawn_capture_thread(
    capture_mode: CaptureMode,
    channels: CaptureThreadChannels,
    manifest: NetsukeManifest,
) -> JoinHandle<Result<CaptureSnapshot>> {
    let CaptureThreadChannels {
        paused_receiver,
        registration_started_sender,
        summary_callback_sender,
        installed_sender,
        finished_receiver,
    } = channels;
    let capture_channels = CaptureModeChannels {
        registration_started_sender,
        installed_sender,
        finished_receiver,
    };
    thread::spawn(move || {
        paused_receiver
            .recv_timeout(THREAD_DEADLINE)
            .context("wait for T1 to pause the summary registration")?;
        crate::test_tracing_capture::with_manifest_summary_registration_signal(
            summary_callback_sender,
            || capture_with_mode(capture_mode, &capture_channels, &manifest),
        )
    })
}

/// Capture T2 through the selected dispatch-installation boundary.
fn capture_with_mode(
    capture_mode: CaptureMode,
    channels: &CaptureModeChannels,
    manifest: &NetsukeManifest,
) -> Result<CaptureSnapshot> {
    match capture_mode {
        CaptureMode::Unmitigated => capture_unmitigated(channels, manifest),
        CaptureMode::Mitigated => capture_mitigated(channels, manifest),
        CaptureMode::PublicHelper => capture_public_helper(channels, manifest),
    }
}

/// Capture with an uninstalled subscriber to reproduce the stale-interest race.
fn capture_unmitigated(
    channels: &CaptureModeChannels,
    manifest: &NetsukeManifest,
) -> Result<CaptureSnapshot> {
    let (subscriber, captured) =
        crate::test_tracing_capture::uninstalled_test_subscriber(LevelFilter::TRACE);
    channels
        .registration_started_sender
        .send(())
        .context("signal T2 reached Dispatch::new")?;
    let dispatch = tracing::Dispatch::new(subscriber);
    tracing::dispatcher::with_default(&dispatch, || {
        capture_manifest_after_install(
            &channels.installed_sender,
            &channels.finished_receiver,
            manifest,
            || CaptureSnapshot {
                events: captured.snapshot(),
                span_fields: captured.span_fields("runner.manifest.structure"),
            },
        )
    })
}

/// Capture with the anchored Netsuke helper and report its registration boundary.
fn capture_mitigated(
    channels: &CaptureModeChannels,
    manifest: &NetsukeManifest,
) -> Result<CaptureSnapshot> {
    crate::test_tracing_capture::with_test_subscriber_at_registration(
        LevelFilter::TRACE,
        || {
            channels
                .registration_started_sender
                .send(())
                .context("signal T2 reached Dispatch::new")
        },
        |captured| {
            capture_manifest_after_install(
                &channels.installed_sender,
                &channels.finished_receiver,
                manifest,
                || CaptureSnapshot {
                    events: captured.snapshot(),
                    span_fields: captured.span_fields("runner.manifest.structure"),
                },
            )
        },
    )
}

/// Capture through the public test-support helper and observe its registration.
fn capture_public_helper(
    channels: &CaptureModeChannels,
    manifest: &NetsukeManifest,
) -> Result<CaptureSnapshot> {
    let registration_started_sender = channels.registration_started_sender.clone();
    let registration_signal_failed = Rc::new(Cell::new(false));
    let hook_signal_failed = Rc::clone(&registration_signal_failed);
    let capture_result = test_support::tracing_capture::with_test_subscriber_registration_hooks(
        move || hook_signal_failed.set(registration_started_sender.send(()).is_err()),
        notify_summary_registration,
        || {
            capture_with_public_subscriber(
                &channels.installed_sender,
                &channels.finished_receiver,
                manifest,
            )
        },
    );
    ensure!(
        !registration_signal_failed.get(),
        "T1 dropped the registration gate before T2 reached Dispatch::new"
    );
    capture_result
}

/// Install the public thread-local helper and snapshot its captured events.
fn capture_with_public_subscriber(
    installed_sender: &Sender<()>,
    finished_receiver: &Receiver<()>,
    manifest: &NetsukeManifest,
) -> Result<CaptureSnapshot> {
    test_support::tracing_capture::with_test_subscriber(LevelFilter::TRACE, |captured| {
        capture_manifest_after_install(installed_sender, finished_receiver, manifest, || {
            CaptureSnapshot {
                events: captured.snapshot(),
                span_fields: Vec::new(),
            }
        })
    })
}

/// Signal the test gate when T2 attempts to register the manifest summary callsite.
fn notify_summary_registration(metadata: &'static tracing::Metadata<'static>) {
    if crate::test_tracing_capture::is_manifest_summary_metadata(metadata) {
        crate::test_tracing_capture::signal_summary_registration();
    }
}

/// Signal dispatch installation and snapshot the production emission after T1 ends.
fn capture_manifest_after_install(
    installed_sender: &Sender<()>,
    finished_receiver: &Receiver<()>,
    manifest: &NetsukeManifest,
    snapshot: impl FnOnce() -> CaptureSnapshot,
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
    Ok(snapshot())
}
