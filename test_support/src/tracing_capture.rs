//! Helpers for capturing structured tracing events in tests.

use std::sync::{Arc, Mutex};
use tracing::{
    Event, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{
    Layer, filter::LevelFilter, layer::Context as LayerContext, prelude::*, registry::LookupSpan,
};

/// Captured tracing event fields.
#[derive(Debug, Clone, Default)]
pub struct CapturedEvents {
    fields: Arc<Mutex<Vec<String>>>,
}

impl CapturedEvents {
    /// Return a snapshot of all captured event fields.
    ///
    /// # Panics
    ///
    /// Panics if another thread panicked while holding the capture lock.
    #[must_use]
    pub fn snapshot(&self) -> Vec<String> {
        self.fields.lock().expect("captured events lock").clone()
    }
}

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
            .expect("captured events lock")
            .push(visitor.fields.join(" "));
    }
}

#[derive(Debug, Default)]
struct FieldVisitor {
    fields: Vec<String>,
}

impl Visit for FieldVisitor {
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields.push(format!("{}={value}", field.name()));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields.push(format!("{}={value}", field.name()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields.push(format!("{}={value}", field.name()));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields.push(format!("{}={value:?}", field.name()));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields.push(format!("{}={value:?}", field.name()));
    }
}

/// Run `test` with a temporary tracing subscriber that captures events.
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
