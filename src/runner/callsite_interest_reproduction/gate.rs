//! Gate one tracing subscriber callback without changing its interest result.

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

/// Configure the bounded wait for T2's real registration callback.
#[derive(Clone, Copy)]
pub(super) struct RegistrationGateOptions {
    /// Maximum time T1 waits for registration or installation.
    pub(super) deadline: Duration,
    /// Whether reaching the deadline before T2 registers is the expected path.
    pub(super) attempt_timeout_is_expected: bool,
}

/// Hold the gate outcome and phase channels for the reproduction worker.
pub(super) struct RegistrationGate {
    paused_sender: Sender<()>,
    attempted_receiver: Mutex<Receiver<()>>,
    installed_receiver: Mutex<Receiver<()>>,
    deadline: Duration,
    attempt_timeout_is_expected: bool,
    first_summary_registration: AtomicBool,
    was_paused: AtomicBool,
    was_attempted: AtomicBool,
    release_cause: Mutex<Option<&'static str>>,
    failure: Mutex<Option<String>>,
}

impl RegistrationGate {
    /// Build a gate that waits for the real registration callback or deadline.
    pub(super) fn new(
        paused_sender: Sender<()>,
        attempted_receiver: Receiver<()>,
        installed_receiver: Receiver<()>,
        options: RegistrationGateOptions,
    ) -> Self {
        Self {
            paused_sender,
            attempted_receiver: Mutex::new(attempted_receiver),
            installed_receiver: Mutex::new(installed_receiver),
            deadline: options.deadline,
            attempt_timeout_is_expected: options.attempt_timeout_is_expected,
            first_summary_registration: AtomicBool::new(false),
            was_paused: AtomicBool::new(false),
            was_attempted: AtomicBool::new(false),
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
        let attempt = self
            .attempted_receiver
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .recv_timeout(self.deadline);
        match attempt {
            Ok(()) => self.was_attempted.store(true, Ordering::Release),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let cause = if self.attempt_timeout_is_expected {
                    "deadline"
                } else {
                    self.store_failure(
                        "T2 did not reach summary callsite registration before the deadline"
                            .to_owned(),
                    );
                    "attempt-timeout"
                };
                *self
                    .release_cause
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner) = Some(cause);
                return;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.store_failure("the T2 registration-callback signal disconnected".to_owned());
                *self
                    .release_cause
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner) = Some("attempt-disconnected");
                return;
            }
        }
        let result = self
            .installed_receiver
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .recv_timeout(self.deadline);
        let cause = match result {
            Ok(()) => "installed",
            Err(mpsc::RecvTimeoutError::Timeout) => "deadline",
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.store_failure("the installed-phase signal disconnected".to_owned());
                "disconnected"
            }
        };
        *self
            .release_cause
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(cause);
    }

    /// Store a fixture failure for the worker thread to report after the gate opens.
    fn store_failure(&self, message: String) {
        *self.failure.lock().unwrap_or_else(PoisonError::into_inner) = Some(message);
    }

    /// Report whether the T1 registration gate was reached.
    pub(super) fn was_paused(&self) -> bool {
        self.was_paused.load(Ordering::Acquire)
    }

    /// Report whether T2 reached the capture layer's summary registration callback.
    pub(super) fn was_attempted(&self) -> bool {
        if self.was_attempted.load(Ordering::Acquire) {
            return true;
        }
        let observed = self
            .attempted_receiver
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .try_recv()
            .is_ok();
        if observed {
            self.was_attempted.store(true, Ordering::Release);
        }
        observed
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
