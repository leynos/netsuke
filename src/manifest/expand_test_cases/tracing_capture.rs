//! Tracing capture tests for manifest expansion filtering.

use super::*;
use anyhow::{Context, Result};
use minijinja::Environment;
use std::sync::{Arc, Mutex};
use tracing::{
    Event, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{
    Layer, filter::LevelFilter, layer::Context as LayerContext, prelude::*, registry::LookupSpan,
};

#[derive(Debug, Clone, Default)]
struct CapturedEvents {
    fields: Arc<Mutex<Vec<String>>>,
}

impl CapturedEvents {
    fn snapshot(&self) -> Vec<String> {
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

fn with_test_subscriber<T>(test: impl FnOnce(CapturedEvents) -> T) -> T {
    let layer = CapturedEventsLayer::default();
    let captured = CapturedEvents {
        fields: Arc::clone(&layer.events),
    };
    let subscriber = tracing_subscriber::registry().with(layer.with_filter(LevelFilter::DEBUG));
    tracing::subscriber::with_default(subscriber, || test(captured))
}

#[test]
fn expand_foreach_emits_debug_event_for_filtered_entry() -> Result<()> {
    with_test_subscriber(|captured| {
        let env = Environment::new();
        let when_expr = "secret_token == 'literal-secret'";
        let yaml = format!(
            "targets:
  - name: skipped-target-secret
    command: echo skipped
    when: {when_expr}"
        );
        let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)?;

        let stats = expand_foreach(&mut doc, &env)?;
        let events = captured.snapshot();

        anyhow::ensure!(
            stats.filtered_targets == 1,
            "expected one filtered target: {stats:?}"
        );
        let expected_hash = entry_name_hash("skipped-target-secret");
        let event = events
            .iter()
            .find(|event| {
                event.contains("filtered manifest entry by when expression")
                    && event.contains("section=\"targets\"")
                    && event.contains(&format!("entry_name_hash=\"{expected_hash}\""))
            })
            .with_context(|| format!("expected filtered-entry debug event in {events:?}"))?;
        anyhow::ensure!(
            event.contains("section=\"targets\""),
            "debug event should include section field: {event}"
        );
        anyhow::ensure!(
            event.contains(&format!("entry_name_hash=\"{expected_hash}\"")),
            "debug event should include bounded entry name hash: {event}"
        );
        anyhow::ensure!(
            event.contains(&format!("when_expression_len={}", when_expr.len())),
            "debug event should include expression length: {event}"
        );
        anyhow::ensure!(
            event.contains("when_result=false"),
            "debug event should include false when result: {event}"
        );
        anyhow::ensure!(
            !event.contains("skipped-target-secret"),
            "debug event should not include raw entry name: {event}"
        );
        anyhow::ensure!(
            !event.contains("secret_token") && !event.contains("literal-secret"),
            "debug event should not include raw when expression: {event}"
        );
        anyhow::ensure!(
            events
                .iter()
                .all(|captured_event| !captured_event.contains("entry_name=")
                    && !captured_event.contains("when_expression=")),
            "structured logs should not include raw entry_name or when_expression fields: {events:?}"
        );
        Ok(())
    })
}
