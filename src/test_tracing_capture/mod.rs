//! Helpers for capturing structured tracing events in tests.

use std::{
    cell::RefCell,
    collections::BTreeMap,
    sync::{Arc, Mutex, PoisonError, mpsc::Sender},
};
use tracing::{
    Event, Metadata, Subscriber,
    field::{Field, Visit},
    subscriber::Interest,
};
use tracing_subscriber::{
    Layer, filter::LevelFilter, layer::Context as LayerContext, prelude::*, registry::LookupSpan,
};

/// Name of the bounded configuration-discovery span.
const DISCOVERY_SPAN: &str = "collect_diag_file_layers";

thread_local! {
    static SUMMARY_REGISTRATION_SIGNAL: RefCell<Option<Sender<()>>> = const {
        RefCell::new(None)
    };
}

/// Observe the real registration callback for the manifest summary callsite.
///
/// The observer is scoped to one thread and one capture setup so the gated
/// worker can tell when `tracing-core` actually reaches its callsite rebuild.
/// It does not install or alter a dispatcher.
///
/// # Examples
///
/// ```ignore
/// let captured = with_manifest_summary_registration_signal(sender, || {
///     with_test_subscriber(LevelFilter::TRACE, |events| events.snapshot())
/// });
/// ```
pub fn with_manifest_summary_registration_signal<T>(
    sender: Sender<()>,
    install_capture: impl FnOnce() -> T,
) -> T {
    let previous = SUMMARY_REGISTRATION_SIGNAL.with(|signal| signal.replace(Some(sender)));
    let _scope = SummaryRegistrationSignalScope { previous };
    install_capture()
}

/// Restore the prior callsite observer after a scoped capture setup finishes.
struct SummaryRegistrationSignalScope {
    previous: Option<Sender<()>>,
}

impl Drop for SummaryRegistrationSignalScope {
    fn drop(&mut self) {
        SUMMARY_REGISTRATION_SIGNAL.with(|signal| {
            drop(signal.replace(self.previous.take()));
        });
    }
}

/// Match only the production manifest-summary event callsite.
///
/// # Examples
///
/// ```ignore
/// assert!(is_manifest_summary_metadata(&summary_metadata));
/// ```
pub(crate) fn is_manifest_summary_metadata(metadata: &Metadata<'_>) -> bool {
    metadata.is_event()
        && metadata.level() == &tracing::Level::TRACE
        && metadata.target() == "netsuke::runner::manifest_structure_telemetry"
        && metadata.fields().field("message").is_some()
}

/// Captured tracing event fields.
///
/// Obtain an instance through [`with_test_subscriber`]; it shares the capture
/// buffer with the installed layer.
#[derive(Debug, Clone)]
pub struct CapturedEvents {
    fields: Arc<Mutex<Vec<String>>>,
    span_fields: Arc<Mutex<BTreeMap<String, Vec<String>>>>,
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

    /// Return the fields carried by every instance of a named span.
    ///
    /// Fields are keyed by span name, so a span created at creation time with
    /// concrete values and one that records them later are both visible. A
    /// value declared `field::Empty` contributes nothing until it is recorded,
    /// which is what lets an assertion distinguish a populated field from one
    /// the code under test never set.
    #[must_use]
    pub fn span_fields(&self, span_name: &str) -> Vec<String> {
        self.span_fields
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(span_name)
            .cloned()
            .unwrap_or_default()
    }

    /// Return fields carried by the bounded configuration-discovery span.
    #[must_use]
    pub fn discovery_span_fields(&self) -> Vec<String> {
        self.span_fields(DISCOVERY_SPAN)
    }
}

/// [`Layer`] that appends each event's rendered fields to a shared buffer.
///
/// The buffer is shared with the [`CapturedEvents`] handle returned to the test,
/// so both observe the same captured events.
#[derive(Debug, Clone, Default)]
struct CapturedEventsLayer {
    events: Arc<Mutex<Vec<String>>>,
    span_fields: Arc<Mutex<BTreeMap<String, Vec<String>>>>,
}

impl CapturedEventsLayer {
    /// Append `fields` under `span_name`, ignoring an empty recording.
    fn capture_span_fields(&self, span_name: &str, fields: Vec<String>) {
        if fields.is_empty() {
            return;
        }
        self.span_fields
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(span_name.to_owned())
            .or_default()
            .extend(fields);
    }
}

impl<S> Layer<S> for CapturedEventsLayer
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    /// Signal the scoped observer when tracing registers the summary callsite.
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        if is_manifest_summary_metadata(metadata) {
            signal_summary_registration();
        }
        Interest::always()
    }

    /// Render `event`'s fields and append them to the shared buffer.
    fn on_event(&self, event: &Event<'_>, _ctx: LayerContext<'_, S>) {
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(visitor.fields.join(" "));
    }

    /// Capture the fields a span is created with.
    ///
    /// A field given a value at creation never reaches `on_record`, so a span
    /// declaring one up front would otherwise be invisible to these tests. An
    /// `Empty` placeholder records nothing here, and is picked up later if and
    /// when the span sets it.
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        _id: &tracing::span::Id,
        _ctx: LayerContext<'_, S>,
    ) {
        let mut visitor = FieldVisitor::default();
        attrs.record(&mut visitor);
        self.capture_span_fields(attrs.metadata().name(), visitor.fields);
    }

    /// Capture the fields a span records after creation.
    fn on_record(
        &self,
        id: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        ctx: LayerContext<'_, S>,
    ) {
        let Some(span) = ctx.span(id) else {
            return;
        };
        let mut visitor = FieldVisitor::default();
        values.record(&mut visitor);
        self.capture_span_fields(span.metadata().name(), visitor.fields);
    }
}

/// Notify the reproduction worker once the capture layer registers the summary.
fn signal_summary_registration() {
    let registration_sender = SUMMARY_REGISTRATION_SIGNAL.with(|signal| signal.borrow_mut().take());
    if let Some(sender) = registration_sender {
        sender.send(()).unwrap_or(());
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
/// thread-local default, so events from other threads are not captured. That
/// limitation is real but does not explain the lost event: production emission
/// is synchronous, and two inert test dispatches keep `tracing-core` from
/// publishing a stale single-registrar `never` interest during concurrent
/// subscriber registration. The anchors are registrars only; this helper does
/// not install a global default.
pub fn with_test_subscriber<T>(
    level_filter: LevelFilter,
    test: impl FnOnce(CapturedEvents) -> T,
) -> T {
    let result = with_test_subscriber_at_registration(
        level_filter,
        || Ok::<(), std::convert::Infallible>(()),
        |captured| Ok(test(captured)),
    );
    match result {
        Ok(value) => value,
        Err(never) => match never {},
    }
}

/// Build a capture subscriber and handle without installing the subscriber.
///
/// This test-internal seam lets subprocess scheduling tests decide precisely
/// when the dispatch is installed while preserving the normal capture layer.
/// Direct use is restricted to the issue 927 reproduction worker; ordinary
/// tests must use [`with_test_subscriber`], which registers interest anchors.
pub(crate) fn uninstalled_test_subscriber(
    level_filter: LevelFilter,
) -> (impl Subscriber + Send + Sync + 'static, CapturedEvents) {
    let layer = CapturedEventsLayer::default();
    let captured = CapturedEvents {
        fields: Arc::clone(&layer.events),
        span_fields: Arc::clone(&layer.span_fields),
    };
    let subscriber = tracing_subscriber::registry().with(layer.with_filter(level_filter));
    (subscriber, captured)
}

/// Install a filtered capture after an optional registration-boundary hook.
///
/// The hook runs after the complete subscriber has been built and immediately
/// before `Dispatch::new`, which enters `tracing-core`'s
/// `callsite::register_dispatch` path. The regular capture helper delegates
/// here with an infallible no-op hook; the issue 927 worker uses the hook to
/// record its controlled registration schedule.
///
/// # Errors
///
/// Returns the error produced by the registration-boundary hook or capture callback.
#[cfg(test)]
pub(crate) fn with_test_subscriber_at_registration<T, E>(
    level_filter: LevelFilter,
    registration_boundary: impl FnOnce() -> Result<(), E>,
    test: impl FnOnce(CapturedEvents) -> Result<T, E>,
) -> Result<T, E> {
    test_support::tracing_capture::ensure_test_interest_anchors_registered();
    let (subscriber, captured) = uninstalled_test_subscriber(level_filter);
    registration_boundary()?;
    let dispatch = tracing::Dispatch::new(subscriber);
    tracing::dispatcher::with_default(&dispatch, || test(captured))
}

#[cfg(test)]
mod tests;
