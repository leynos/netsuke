//! Tracing-event capture for `test_support`'s own unit tests.
//!
//! This mirrors the netsuke crate's `test_tracing_capture`, which cannot be
//! reused here: it is `#[cfg(test)] pub(crate)` and so is not part of the
//! netsuke library `test_support` links against. The helper is confined to
//! this crate's test builds and is not part of the published test surface.

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
pub(crate) struct CapturedEvents {
    fields: Arc<Mutex<Vec<String>>>,
}

impl CapturedEvents {
    /// Return a snapshot of all captured event fields.
    ///
    /// Recovers the inner guard if the capture lock was poisoned, so a panic in
    /// another test thread does not cascade into this snapshot.
    pub(crate) fn snapshot(&self) -> Vec<String> {
        self.fields
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// [`Layer`] that appends each event's rendered fields to a shared buffer.
#[derive(Debug, Clone, Default)]
struct CapturedEventsLayer {
    events: Arc<Mutex<Vec<String>>>,
}

impl<S> Layer<S> for CapturedEventsLayer
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
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

impl Visit for FieldVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields.push(format!("{}={value:?}", field.name()));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields.push(format!("{}={value:?}", field.name()));
    }
}

/// Run `test` with a temporary tracing subscriber that captures events.
///
/// [`tracing::subscriber::with_default`] installs the subscriber as a
/// thread-local default, so only events emitted on the calling thread are
/// captured; events from threads spawned inside `test` are not recorded.
pub(crate) fn with_test_subscriber<T>(
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
