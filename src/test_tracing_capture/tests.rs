//! Tests for the capture handle's shared-state guarantees.
//!
//! These live inside the module because poisoning the capture lock needs
//! the private `Arc<Mutex<_>>`; no public API can reach it.

use super::*;
use std::sync::{Arc, Barrier};

/// Snapshot `captured` from another thread once every reader has arrived.
///
/// Extracted so the spawning closure stays shallow and the cloned barrier
/// does not shadow the original binding.
fn spawn_reader(
    captured: &CapturedEvents,
    barrier: &Arc<Barrier>,
) -> std::thread::JoinHandle<Vec<String>> {
    let handle = captured.clone();
    let gate = Arc::clone(barrier);
    std::thread::spawn(move || {
        gate.wait();
        handle.snapshot()
    })
}

/// Cloned handles observe the same buffer from other threads.
///
/// Only `snapshot()` runs off-thread: the subscriber is a thread-local
/// default, so a spawned thread could not emit captured events anyway.
#[test]
fn snapshot_is_readable_concurrently_from_cloned_handles() {
    let events = with_test_subscriber(LevelFilter::TRACE, |captured| {
        tracing::info!(step = 1, "first");
        let barrier = Arc::new(Barrier::new(4));
        let readers: Vec<_> = (0..3).map(|_| spawn_reader(&captured, &barrier)).collect();
        barrier.wait();
        let snapshots: Vec<_> = readers
            .into_iter()
            .map(|reader| reader.join().expect("reader thread should not panic"))
            .collect();
        (captured.snapshot(), snapshots)
    });

    let (final_snapshot, concurrent) = events;
    assert_eq!(final_snapshot.len(), 1, "one event was emitted");
    for snapshot in concurrent {
        assert!(
            snapshot.iter().all(|event| final_snapshot.contains(event)),
            "concurrent snapshot must be a subset of the final buffer"
        );
    }
}

/// A poisoned capture lock still yields a snapshot rather than panicking.
#[test]
fn snapshot_recovers_from_a_poisoned_lock() {
    let captured = CapturedEvents {
        fields: Arc::new(Mutex::new(vec!["seeded=1".to_owned()])),
        span_fields: Arc::new(Mutex::new(BTreeMap::new())),
    };

    let poisoner = Arc::clone(&captured.fields);
    let handle = std::thread::spawn(move || {
        let _guard = poisoner.lock().expect("lock should be held");
        panic!("poison the capture lock");
    });
    assert!(handle.join().is_err(), "the thread should have panicked");
    assert!(
        captured.fields.is_poisoned(),
        "the lock should now be poisoned"
    );

    assert_eq!(
        captured.snapshot(),
        vec!["seeded=1".to_owned()],
        "snapshot should recover the guard instead of panicking"
    );
}

/// A nested subscriber shadows the outer one for its scope.
///
/// `with_default` keeps a thread-local stack, so events dispatch only to
/// the innermost subscriber; the outer handle never sees them.
#[test]
fn nested_subscribers_capture_only_their_own_scope() {
    let (outer, inner) = with_test_subscriber(LevelFilter::TRACE, |outer_captured| {
        tracing::info!(scope = "outer_before", "outer");
        let inner_events = with_test_subscriber(LevelFilter::TRACE, |inner_captured| {
            tracing::info!(scope = "inner", "inner");
            inner_captured.snapshot()
        });
        tracing::info!(scope = "outer_after", "outer");
        (outer_captured.snapshot(), inner_events)
    });

    assert_eq!(inner.len(), 1, "inner scope captures only its own event");
    assert!(
        inner.first().is_some_and(|event| event.contains("inner")),
        "inner snapshot should hold the nested event: {inner:?}"
    );
    assert_eq!(
        outer.len(),
        2,
        "outer captures its own events only: {outer:?}"
    );
    assert!(
        outer.iter().all(|event| !event.contains("scope=\"inner\"")),
        "outer must not see the nested event: {outer:?}"
    );
}

/// The span-field API separates names, captures either recording point,
/// and reports a field the code never set as absent rather than empty.
#[test]
fn span_fields_are_captured_by_name_and_recording_point() {
    let (discovery, other, unset) = with_test_subscriber(LevelFilter::TRACE, |captured| {
        let discovery =
            tracing::trace_span!("collect_diag_file_layers", outcome = tracing::field::Empty,);
        discovery.record("outcome", "success");
        // A value supplied at creation never reaches `on_record`, so the
        // layer has to capture `on_new_span` as well to see this one. The
        // recorded field is declared `Empty` first: `record` resolves its
        // name against the span's declared set and drops a name the span
        // never declared, which would leave `on_record` unexercised.
        let other = tracing::trace_span!(
            "other_span",
            at_creation = "visible",
            later = tracing::field::Empty,
        );
        other.record("later", "also_visible");
        let unset = tracing::trace_span!("unset_span", never_set = tracing::field::Empty);
        let _guard = unset.enter();
        (
            captured.discovery_span_fields(),
            captured.span_fields("other_span"),
            captured.span_fields("unset_span"),
        )
    });

    assert_eq!(discovery, vec!["outcome=\"success\"".to_owned()]);
    assert_eq!(
        other,
        vec![
            "at_creation=\"visible\"".to_owned(),
            "later=\"also_visible\"".to_owned()
        ],
        "a named span's creation and recorded fields both belong to it"
    );
    assert!(
        unset.is_empty(),
        "an Empty field never recorded must read as absent: {unset:?}"
    );
}

/// A same-callsite DEBUG-to-TRACE transition refreshes cached interest.
#[test]
fn callsite_interest_refreshes_when_capture_level_becomes_trace() {
    fn emit_transition_event() {
        tracing::trace!(transition = true, "interest transition");
    }

    test_support::tracing_capture::ensure_test_interest_anchors_registered();
    let (subscriber, captured) = uninstalled_test_subscriber(LevelFilter::DEBUG);
    let transition_interests = Arc::new(Mutex::new(Vec::new()));
    let debug_subscriber = InterestObservingSubscriber {
        inner: subscriber,
        transition_interests: Arc::clone(&transition_interests),
    };
    let debug_events = tracing::subscriber::with_default(debug_subscriber, || {
        emit_transition_event();
        captured.snapshot()
    });
    assert!(
        debug_events.is_empty(),
        "DEBUG capture must filter the TRACE event: {debug_events:?}"
    );
    let observed_interests = transition_interests
        .lock()
        .expect("callsite interest observations should remain available");
    assert!(
        !observed_interests.is_empty() && observed_interests.iter().all(|is_never| *is_never),
        "the DEBUG subscriber must register the transition callsite with never interest only: {observed_interests:?}"
    );

    let trace_events = with_test_subscriber(LevelFilter::TRACE, |trace_capture| {
        emit_transition_event();
        trace_capture.snapshot()
    });
    assert_eq!(
        trace_events.len(),
        1,
        "TRACE capture must observe the same callsite after the level transition"
    );
}

/// Delegate capture behaviour while observing the transition callsite's interest.
struct InterestObservingSubscriber<S> {
    inner: S,
    transition_interests: Arc<Mutex<Vec<bool>>>,
}

impl<S: Subscriber> Subscriber for InterestObservingSubscriber<S> {
    fn on_register_dispatch(&self, dispatch: &tracing::dispatcher::Dispatch) {
        self.inner.on_register_dispatch(dispatch);
    }

    fn register_callsite(
        &self,
        metadata: &'static tracing::Metadata<'static>,
    ) -> tracing::subscriber::Interest {
        let interest = self.inner.register_callsite(metadata);
        if metadata.target() == module_path!() && metadata.fields().field("transition").is_some() {
            self.transition_interests
                .lock()
                .expect("callsite interest observations should remain available")
                .push(interest.is_never());
        }
        interest
    }

    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        self.inner.enabled(metadata)
    }

    fn max_level_hint(&self) -> Option<LevelFilter> {
        // Keep TRACE callsites eligible for registration while the filter
        // records their DEBUG-phase `never` interest.
        Some(LevelFilter::TRACE)
    }

    fn new_span(&self, span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        self.inner.new_span(span)
    }

    fn record(&self, span: &tracing::span::Id, values: &tracing::span::Record<'_>) {
        self.inner.record(span, values);
    }

    fn record_follows_from(&self, span: &tracing::span::Id, follows: &tracing::span::Id) {
        self.inner.record_follows_from(span, follows);
    }

    fn event_enabled(&self, event: &tracing::Event<'_>) -> bool {
        self.inner.event_enabled(event)
    }

    fn event(&self, event: &tracing::Event<'_>) {
        self.inner.event(event);
    }

    fn enter(&self, span: &tracing::span::Id) {
        self.inner.enter(span);
    }

    fn exit(&self, span: &tracing::span::Id) {
        self.inner.exit(span);
    }

    fn clone_span(&self, id: &tracing::span::Id) -> tracing::span::Id {
        self.inner.clone_span(id)
    }

    fn try_close(&self, id: tracing::span::Id) -> bool {
        self.inner.try_close(id)
    }

    // The test emits only an event, so the wrapper keeps the trait's default
    // unknown current-span state rather than exposing tracing-core's private
    // re-exported return type just to delegate an unused query.
}

/// Anchoring test registrars leaves the thread's default no-op dispatch intact.
#[test]
fn anchors_do_not_install_a_default_dispatch() {
    test_support::tracing_capture::ensure_test_interest_anchors_registered();
    let is_noop = tracing::dispatcher::get_default(|dispatch| {
        dispatch.is::<tracing::subscriber::NoSubscriber>()
    });
    assert!(
        is_noop,
        "the caller's default dispatch must remain the no-op"
    );
}
