//! Helpers for capturing structured tracing events in tests.

use std::sync::{Arc, Mutex, PoisonError};
use tracing::{
    Event, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{
    Layer, filter::LevelFilter, layer::Context as LayerContext, prelude::*, registry::LookupSpan,
};

/// Captured tracing event fields.
///
/// Obtain an instance through [`with_test_subscriber`]; it shares the capture
/// buffer with the installed layer.
#[derive(Debug, Clone)]
pub struct CapturedEvents {
    fields: Arc<Mutex<Vec<String>>>,
}

impl CapturedEvents {
    /// Return a snapshot of all captured event fields.
    ///
    /// Recovers the inner guard if the capture lock was poisoned, so a panic in
    /// another test thread does not cascade into this snapshot.
    #[must_use]
    pub fn snapshot(&self) -> Vec<String> {
        self.fields
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// [`Layer`] that appends each event's rendered fields to a shared buffer.
///
/// The buffer is shared with the [`CapturedEvents`] handle returned to the test,
/// so both observe the same captured events.
#[derive(Debug, Clone, Default)]
struct CapturedEventsLayer {
    events: Arc<Mutex<Vec<String>>>,
}

impl<S> Layer<S> for CapturedEventsLayer
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    /// Render `event`'s fields and append them to the shared buffer.
    fn on_event(&self, event: &Event<'_>, _ctx: LayerContext<'_, S>) {
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(visitor.fields.join(" "));
    }
}

/// [`Visit`] implementation that renders each recorded field as `name=value`.
#[derive(Debug, Default)]
struct FieldVisitor {
    fields: Vec<String>,
}

/// Each `record_*` method renders its field as `name=value`, quoting strings and
/// `Debug` values so assertions can match exact field renderings.
impl Visit for FieldVisitor {
    /// Record a boolean field unquoted, as `name=true` or `name=false`.
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields.push(format!("{}={value}", field.name()));
    }

    /// Record a signed integer field unquoted.
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields.push(format!("{}={value}", field.name()));
    }

    /// Record an unsigned integer field unquoted.
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields.push(format!("{}={value}", field.name()));
    }

    /// Record a string field quoted, so assertions can match exact values.
    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields.push(format!("{}={value:?}", field.name()));
    }

    /// Record any remaining field through its `Debug` rendering.
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields.push(format!("{}={value:?}", field.name()));
    }
}

/// Run `test` with a temporary tracing subscriber that captures events.
///
/// [`tracing::subscriber::with_default`] installs the subscriber as a
/// thread-local default, so only events emitted on the calling thread are
/// captured; events from threads spawned inside `test` are not recorded.
pub fn with_test_subscriber<T>(
    level_filter: LevelFilter,
    test: impl FnOnce(CapturedEvents) -> T,
) -> T {
    let layer = CapturedEventsLayer::default();
    let captured = CapturedEvents {
        fields: Arc::clone(&layer.events),
    };
    let subscriber = tracing_subscriber::registry().with(layer.with_filter(level_filter));
    tracing::subscriber::with_default(subscriber, || test(captured))
}

#[cfg(test)]
mod tests {
    //! Tests for the capture handle's shared-state guarantees.
    //!
    //! These live inside the module because poisoning the capture lock needs
    //! the private `Arc<Mutex<_>>`; no public API can reach it.

    use super::*;
    use std::sync::{Arc, Barrier};

    /// Cloned handles observe the same buffer from other threads.
    ///
    /// Only `snapshot()` runs off-thread: the subscriber is a thread-local
    /// default, so a spawned thread could not emit captured events anyway.
    #[test]
    fn snapshot_is_readable_concurrently_from_cloned_handles() {
        let events = with_test_subscriber(LevelFilter::TRACE, |captured| {
            tracing::info!(step = 1, "first");
            let barrier = Arc::new(Barrier::new(4));
            let readers: Vec<_> = (0..3)
                .map(|_| {
                    let handle = captured.clone();
                    let barrier = Arc::clone(&barrier);
                    std::thread::spawn(move || {
                        barrier.wait();
                        handle.snapshot()
                    })
                })
                .collect();
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
            inner[0].contains("inner"),
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
}
