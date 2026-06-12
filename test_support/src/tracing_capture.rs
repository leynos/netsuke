//! Capture tracing events emitted during a test closure.
//!
//! Installs a thread-local subscriber for the duration of a closure and
//! records each event's fields as a single `name=value`-joined string, so
//! tests can assert on structured log output without touching the global
//! subscriber. Intended for any test that verifies observability behaviour;
//! prefer this over per-test bespoke capture layers.
//!
//! # Examples
//!
//! ```rust
//! use test_support::tracing_capture::with_test_subscriber;
//!
//! let events = with_test_subscriber(|captured| {
//!     tracing::debug!(layer = "defaults", "applied layer");
//!     captured.snapshot()
//! });
//! assert!(events.iter().any(|event| event.contains("layer=\"defaults\"")));
//! ```

use std::sync::{Arc, Mutex};
use tracing::{
    Event, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{
    Layer, filter::LevelFilter, layer::Context as LayerContext, prelude::*, registry::LookupSpan,
};

/// Handle for reading events captured by [`with_test_subscriber`].
#[derive(Debug, Clone, Default)]
pub struct CapturedEvents {
    fields: Arc<Mutex<Vec<String>>>,
}

impl CapturedEvents {
    /// Return a copy of the captured event field strings.
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

/// Run `test` with a thread-local subscriber capturing DEBUG+ events.
pub fn with_test_subscriber<T>(test: impl FnOnce(CapturedEvents) -> T) -> T {
    let layer = CapturedEventsLayer::default();
    let captured = CapturedEvents {
        fields: Arc::clone(&layer.events),
    };
    let subscriber = tracing_subscriber::registry().with(layer.with_filter(LevelFilter::DEBUG));
    tracing::subscriber::with_default(subscriber, || test(captured))
}
