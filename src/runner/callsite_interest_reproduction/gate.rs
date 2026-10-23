//! Hold T1 at one callsite while coordinating T2's dispatch-registration phases.

use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
};
use std::time::Duration;

use crate::test_tracing_capture::is_manifest_summary_metadata;
use tracing::{Event, Metadata, Subscriber, dispatcher::Dispatch, span};
use tracing_subscriber::filter::LevelFilter;

pub(super) const GATE_DEADLINE: Duration = Duration::from_millis(250);

/// Carry the phase channels used by the two-thread registration schedule.
pub(super) struct RegistrationGateChannels {
    pub(super) paused_sender: Sender<()>,
    pub(super) registration_started_receiver: Receiver<()>,
    pub(super) summary_callback_receiver: Receiver<()>,
    pub(super) installed_receiver: Receiver<()>,
}

/// Hold the gate outcome and phase channels for the reproduction worker.
pub(super) struct RegistrationGate {
    paused_sender: Sender<()>,
    registration_started_receiver: Mutex<Receiver<()>>,
    summary_callback_receiver: Mutex<Receiver<()>>,
    installed_receiver: Mutex<Receiver<()>>,
    deadline: Duration,
    registration_start_deadline: Duration,
    first_summary_registration: AtomicBool,
    was_paused: AtomicBool,
    registration_started_before_release: AtomicBool,
    summary_callback_before_release: AtomicBool,
    release_cause: Mutex<Option<&'static str>>,
    failure: Mutex<Option<String>>,
}

impl RegistrationGate {
    /// Build a gate with separate registration and callback deadlines.
    ///
    /// Subscriber construction happens before the registration boundary, so it
    /// needs a longer liveness bound than the short callback window used to
    /// distinguish the two schedules.
    pub(super) fn new(
        channels: RegistrationGateChannels,
        deadline: Duration,
        registration_start_deadline: Duration,
    ) -> Self {
        Self {
            paused_sender: channels.paused_sender,
            registration_started_receiver: Mutex::new(channels.registration_started_receiver),
            summary_callback_receiver: Mutex::new(channels.summary_callback_receiver),
            installed_receiver: Mutex::new(channels.installed_receiver),
            deadline,
            registration_start_deadline,
            first_summary_registration: AtomicBool::new(false),
            was_paused: AtomicBool::new(false),
            registration_started_before_release: AtomicBool::new(false),
            summary_callback_before_release: AtomicBool::new(false),
            release_cause: Mutex::new(None),
            failure: Mutex::new(None),
        }
    }

    /// Pause only T1's first registration of the production summary event.
    fn pause_first_summary_registration(&self) {
        if self.first_summary_registration.swap(true, Ordering::AcqRel) {
            return;
        }
        self.was_paused.store(true, Ordering::Release);
        if let Err(error) = self.paused_sender.send(()) {
            self.store_failure(format!("signal the paused phase: {error}"));
        }
        let registration_start = self
            .registration_started_receiver
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .recv_timeout(self.registration_start_deadline);
        match registration_start {
            Ok(()) => self
                .registration_started_before_release
                .store(true, Ordering::Release),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.store_failure(
                    "T2 did not enter Dispatch::new before the registration deadline".to_owned(),
                );
                self.set_release_cause("registration-start-timeout");
                return;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.store_failure("the T2 registration-boundary signal disconnected".to_owned());
                self.set_release_cause("registration-start-disconnected");
                return;
            }
        }

        let summary_callback = self
            .summary_callback_receiver
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .recv_timeout(self.deadline);
        match summary_callback {
            Ok(()) => self
                .summary_callback_before_release
                .store(true, Ordering::Release),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.set_release_cause("deadline");
                return;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.store_failure("the T2 summary-callback signal disconnected".to_owned());
                self.set_release_cause("summary-callback-disconnected");
                return;
            }
        }

        let installation = self
            .installed_receiver
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .recv_timeout(self.deadline);
        let cause = match installation {
            Ok(()) => "installed",
            Err(mpsc::RecvTimeoutError::Timeout) => "deadline",
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.store_failure("the installed-phase signal disconnected".to_owned());
                "disconnected"
            }
        };
        self.set_release_cause(cause);
    }

    /// Store a fixture failure for the worker thread to report after the gate opens.
    fn store_failure(&self, message: String) {
        *self.failure.lock().unwrap_or_else(PoisonError::into_inner) = Some(message);
    }

    /// Record why T1 left the registration gate.
    fn set_release_cause(&self, cause: &'static str) {
        *self
            .release_cause
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(cause);
    }

    /// Report whether the T1 registration gate was reached.
    pub(super) fn was_paused(&self) -> bool {
        self.was_paused.load(Ordering::Acquire)
    }

    /// Report whether T2 entered `Dispatch::new` before T1 left the gate.
    pub(super) fn registration_started_before_release(&self) -> bool {
        self.registration_started_before_release
            .load(Ordering::Acquire)
    }

    /// Report whether the summary callback arrived before T1 left the gate.
    pub(super) fn summary_callback_before_release(&self) -> bool {
        self.summary_callback_before_release.load(Ordering::Acquire)
    }

    /// Consume the summary callback that follows T1's registry-lock release.
    pub(super) fn take_summary_callback_after_release(&self) -> bool {
        // The anchored registration cannot enter Layer::register_callsite until
        // T1 releases the registry read lock held across this gated callback.
        self.summary_callback_receiver
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .try_recv()
            .is_ok()
    }

    /// Return the recorded release cause after both worker threads finish.
    pub(super) fn release_cause(&self) -> Option<&'static str> {
        *self
            .release_cause
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Return any channel failure recorded while the subscriber callback was gated.
    pub(super) fn failure(&self) -> Option<String> {
        self.failure
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// Delegate to a subscriber while gating one first-time callsite registration.
pub(super) struct GatedSubscriber<S> {
    inner: S,
    gate: Arc<RegistrationGate>,
}

impl<S> GatedSubscriber<S> {
    /// Wrap the DEBUG composition while retaining the shared scheduling gate.
    pub(super) fn new(inner: S, gate: Arc<RegistrationGate>) -> Self {
        Self { inner, gate }
    }
}

impl<S> Subscriber for GatedSubscriber<S>
where
    S: Subscriber + Send + Sync,
{
    /// Gate the first summary-event registration and preserve the inner interest.
    fn register_callsite(
        &self,
        metadata: &'static Metadata<'static>,
    ) -> tracing::subscriber::Interest {
        if is_manifest_summary_metadata(metadata) {
            self.gate.pause_first_summary_registration();
        }
        self.inner.register_callsite(metadata)
    }

    /// Delegate enabled checks to the DEBUG composition.
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        self.inner.enabled(metadata)
    }

    /// Delegate span creation to the DEBUG composition.
    fn new_span(&self, attributes: &span::Attributes<'_>) -> span::Id {
        self.inner.new_span(attributes)
    }

    /// Delegate span recording to the DEBUG composition.
    fn record(&self, id: &span::Id, values: &span::Record<'_>) {
        self.inner.record(id, values);
    }

    /// Delegate span relationships to the DEBUG composition.
    fn record_follows_from(&self, id: &span::Id, follows: &span::Id) {
        self.inner.record_follows_from(id, follows);
    }

    /// Delegate event filtering to the DEBUG composition.
    fn event_enabled(&self, event: &Event<'_>) -> bool {
        self.inner.event_enabled(event)
    }

    /// Delegate event delivery to the DEBUG composition.
    fn event(&self, event: &Event<'_>) {
        self.inner.event(event);
    }

    /// Delegate span entry to the DEBUG composition.
    fn enter(&self, id: &span::Id) {
        self.inner.enter(id);
    }

    /// Delegate span exit to the DEBUG composition.
    fn exit(&self, id: &span::Id) {
        self.inner.exit(id);
    }

    /// Delegate dispatch registration to the DEBUG composition.
    fn on_register_dispatch(&self, dispatch: &Dispatch) {
        self.inner.on_register_dispatch(dispatch);
    }

    /// Preserve the DEBUG composition's span reference accounting.
    fn clone_span(&self, id: &span::Id) -> span::Id {
        self.inner.clone_span(id)
    }

    /// Preserve the DEBUG composition's span close notifications.
    fn try_close(&self, id: span::Id) -> bool {
        self.inner.try_close(id)
    }

    /// Keep TRACE callsites from being elided before the registration gate runs.
    ///
    /// The wrapped DEBUG filter still returns its normal interest. A permissive
    /// hint only ensures the macro reaches that callsite-registration decision.
    fn max_level_hint(&self) -> Option<LevelFilter> {
        Some(LevelFilter::TRACE)
    }
}

#[cfg(test)]
mod tests {
    //! Check the gate keeps late registration callbacks out of its deadline window.

    use super::{RegistrationGate, RegistrationGateChannels};
    use std::{sync::mpsc, thread, time::Duration};

    #[test]
    fn delayed_registration_start_is_rejected_by_the_gate() {
        let (paused_sender, paused_receiver) = mpsc::channel();
        let (registration_started_sender, registration_started_receiver) = mpsc::channel();
        let (_summary_callback_sender, summary_callback_receiver) = mpsc::channel();
        let (_installed_sender, installed_receiver) = mpsc::channel();
        let gate = std::sync::Arc::new(RegistrationGate::new(
            RegistrationGateChannels {
                paused_sender,
                registration_started_receiver,
                summary_callback_receiver,
                installed_receiver,
            },
            Duration::from_millis(1),
            Duration::from_millis(1),
        ));
        let worker_gate = std::sync::Arc::clone(&gate);
        let worker = thread::spawn(move || worker_gate.pause_first_summary_registration());

        paused_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("the gate reports that T1 reached the callback");
        worker
            .join()
            .expect("the gate returns when its short deadline expires");
        registration_started_sender
            .send(())
            .expect("the delayed registration signal still has a receiver");

        assert!(!gate.registration_started_before_release());
        assert_eq!(gate.release_cause(), Some("registration-start-timeout"));
        assert!(gate.failure().is_some());
    }
}
