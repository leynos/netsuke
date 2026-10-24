//! Thread-local capture of structured tracing events for integration tests.
//!
//! The capture layer is deliberately local to the calling thread, allowing
//! observability assertions without installing or replacing a global subscriber.
//! Two inert dispatches remain registered to keep concurrent test callsite
//! interest rebuilds conservative; they are never installed as defaults.

use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use tracing::{
    Event, Metadata, Subscriber,
    dispatcher::Dispatch,
    field::{Field, Visit},
    span,
    subscriber::Interest,
};
use tracing_subscriber::{
    Layer, filter::LevelFilter, layer::Context as LayerContext, prelude::*, registry::LookupSpan,
};

/// Events recorded by [`with_test_subscriber`].
#[derive(Debug, Clone)]
pub struct CapturedEvents {
    /// Shared rendered event fields captured by the thread-local subscriber.
    fields: Arc<Mutex<Vec<String>>>,
}

/// Register two permanent inert dispatches to keep test callsite interests conservative.
///
/// `tracing-core` can cache a single registrar's `never` interest while another
/// thread registers a more permissive dispatch. Two retained inert dispatches
/// keep test registrations out of that single-registrar path. They are held as
/// registrars only and are never installed as scoped or global defaults.
///
/// # Examples
///
/// ```
/// use test_support::tracing_capture::ensure_test_interest_anchors_registered;
///
/// ensure_test_interest_anchors_registered();
/// ```
pub fn ensure_test_interest_anchors_registered() {
    static ANCHORS: OnceLock<[Dispatch; 2]> = OnceLock::new();
    let _anchors =
        ANCHORS.get_or_init(|| [Dispatch::new(InterestAnchor), Dispatch::new(InterestAnchor)]);
}

/// Subscriber retained only to keep the callsite registrar set above one.
#[derive(Debug)]
struct InterestAnchor;

impl Subscriber for InterestAnchor {
    /// Reject every callsite in this inert registrar.
    fn register_callsite(&self, _metadata: &'static Metadata<'static>) -> Interest {
        Interest::never()
    }

    /// Report that this subscriber never enables events.
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        false
    }

    /// Return a placeholder identifier for an inert span.
    fn new_span(&self, _attributes: &span::Attributes<'_>) -> span::Id {
        span::Id::from_u64(1)
    }

    /// Ignore values recorded on a span.
    fn record(&self, _span: &span::Id, _values: &span::Record<'_>) {}

    /// Ignore span relationship updates.
    fn record_follows_from(&self, _span: &span::Id, _follows: &span::Id) {}

    /// Ignore events delivered to this registrar.
    fn event(&self, _event: &Event<'_>) {}

    /// Ignore span-entry notifications.
    fn enter(&self, _span: &span::Id) {}

    /// Ignore span-exit notifications.
    fn exit(&self, _span: &span::Id) {}

    /// Report that this subscriber accepts no tracing levels.
    fn max_level_hint(&self) -> Option<LevelFilter> {
        Some(LevelFilter::OFF)
    }
}

impl CapturedEvents {
    /// Return a copy of every event captured so far.
    ///
    /// A poisoned capture buffer remains readable so an earlier failing test
    /// does not cascade into unrelated observability assertions.
    #[must_use]
    pub fn snapshot(&self) -> Vec<String> {
        self.fields
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// Layer that renders each event's fields into the shared capture buffer.
#[derive(Debug, Clone, Default)]
struct CapturedEventsLayer {
    /// Shared destination for fields rendered from each observed event.
    events: Arc<Mutex<Vec<String>>>,
}

impl<S> Layer<S> for CapturedEventsLayer
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    fn on_event(&self, event: &Event<'_>, _context: LayerContext<'_, S>) {
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(visitor.fields.join(" "));
    }
}

/// Visitor that renders tracing fields as stable `name=value` strings.
#[derive(Debug, Default)]
struct FieldVisitor {
    /// Stable `name=value` renderings collected from one tracing event.
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

/// Run `test` with a thread-local subscriber that captures matching events.
///
/// Events emitted by threads spawned within `test` are not captured because
/// `tracing::subscriber::with_default` scopes the subscriber to this thread.
/// The two inert interest anchors protect concurrent callsite registration but
/// do not change that thread-local capture boundary or install a global default.
///
/// # Examples
///
/// ```
/// use test_support::tracing_capture::with_test_subscriber;
/// use tracing_subscriber::filter::LevelFilter;
///
/// let events = with_test_subscriber(LevelFilter::DEBUG, |captured| {
///     tracing::debug!(layer = "defaults", "applied configuration layer");
///     captured.snapshot()
/// });
/// assert!(events.iter().any(|event| event.contains("layer=\"defaults\"")));
/// ```
pub fn with_test_subscriber<T>(
    level_filter: LevelFilter,
    test: impl FnOnce(CapturedEvents) -> T,
) -> T {
    ensure_test_interest_anchors_registered();
    let layer = CapturedEventsLayer::default();
    let captured = CapturedEvents {
        fields: Arc::clone(&layer.events),
    };
    let subscriber = tracing_subscriber::registry().with(layer.with_filter(level_filter));
    tracing::subscriber::with_default(subscriber, || test(captured))
}
