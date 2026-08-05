//! Bounded-telemetry assertions for `render_template`.
//!
//! A local `DebuggingRecorder` captures the counter and histogram without
//! touching the global recorder, and the tracing capture asserts the
//! error event stays free of template content.

use super::super::jinja_macros::{register_macro, render_template};
use crate::ast::MacroDefinition;
use crate::test_tracing_capture::with_test_subscriber;
use anyhow::{Result as AnyResult, ensure};
use metrics_util::MetricKind;
use metrics_util::debugging::{DebugValue, DebuggingRecorder};
use minijinja::{Environment, UndefinedBehavior};
use rstest::{fixture, rstest};
use tracing_subscriber::filter::LevelFilter;

#[fixture]
fn strict_env() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env
}

const RENDERS_TOTAL: &str = "netsuke_manifest_template_renders_total";
const RENDER_DURATION: &str = "netsuke_manifest_template_render_duration_seconds";

/// One drained metrics snapshot; the debugging snapshotter empties
/// histogram samples on read, so it is taken exactly once per test.
type Snapshot = Vec<(
    metrics_util::CompositeKey,
    Option<metrics::Unit>,
    Option<metrics::SharedString>,
    DebugValue,
)>;

/// Run `render` under a local recorder and return its result alongside
/// the single drained snapshot.
fn recorded<T>(render: impl FnOnce() -> T) -> (T, Snapshot) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let value = metrics::with_local_recorder(&recorder, render);
    let snapshot = snapshotter.snapshot().into_vec();
    (value, snapshot)
}

fn counter_value(snapshot: &Snapshot, outcome: &str, has_macro_imports: &str) -> Option<u64> {
    snapshot
        .iter()
        .find_map(|(key, _unit, _description, value)| {
            if key.kind() != MetricKind::Counter || key.key().name() != RENDERS_TOTAL {
                return None;
            }
            let labels: Vec<(String, String)> = key
                .key()
                .labels()
                .map(|label| (label.key().to_owned(), label.value().to_owned()))
                .collect();
            let matches = labels.contains(&("outcome".to_owned(), outcome.to_owned()))
                && labels.contains(&("has_macro_imports".to_owned(), has_macro_imports.to_owned()));
            match value {
                DebugValue::Counter(count) if matches => Some(*count),
                _ => None,
            }
        })
}

fn duration_sample_count(snapshot: &Snapshot) -> usize {
    snapshot
        .iter()
        .find_map(|(key, _unit, _description, value)| {
            if key.kind() != MetricKind::Histogram || key.key().name() != RENDER_DURATION {
                return None;
            }
            match value {
                DebugValue::Histogram(samples) => Some(samples.len()),
                _ => None,
            }
        })
        .unwrap_or_default()
}

#[rstest]
fn imported_macro_render_records_success_telemetry(
    mut strict_env: Environment<'static>,
) -> AnyResult<()> {
    let definition = MacroDefinition {
        signature: "greet(name)".into(),
        body: "Hello {{ name }}".into(),
    };
    register_macro(&mut strict_env, &definition, 0)?;

    let (rendered, snapshot) =
        recorded(|| render_template(&strict_env, "{{ greet('netsuke') }}", &()));

    ensure!(rendered? == "Hello netsuke", "imported macro should render");
    ensure!(
        counter_value(&snapshot, "success", "true") == Some(1),
        "a successful imported-macro render should count once"
    );
    ensure!(
        duration_sample_count(&snapshot) == 1,
        "the render duration should record one sample"
    );
    Ok(())
}

#[rstest]
fn plain_render_records_success_telemetry(strict_env: Environment<'static>) -> AnyResult<()> {
    let (rendered, snapshot) = recorded(|| render_template(&strict_env, "{{ 1 + 1 }}", &()));

    ensure!(rendered? == "2", "the no-import path should render");
    ensure!(
        counter_value(&snapshot, "success", "false") == Some(1),
        "a successful no-import render should count once"
    );
    ensure!(
        duration_sample_count(&snapshot) == 1,
        "the render duration should record one sample"
    );
    Ok(())
}

#[rstest]
fn failed_render_records_error_telemetry_without_template_text(
    strict_env: Environment<'static>,
) -> AnyResult<()> {
    let template = "s3cr3t-sentinel {{ undefined_sentinel_variable }}";

    let (events, snapshot) = {
        let ((result, events), snapshot) = recorded(|| {
            with_test_subscriber(LevelFilter::DEBUG, |captured| {
                let result = render_template(&strict_env, template, &());
                (result, captured.snapshot())
            })
        });
        ensure!(
            result.is_err(),
            "a strict undefined lookup should fail the render"
        );
        (events, snapshot)
    };

    ensure!(
        counter_value(&snapshot, "error", "false") == Some(1),
        "a failed render should count once with the error outcome"
    );
    ensure!(
        duration_sample_count(&snapshot) == 1,
        "the failed render duration should still record one sample"
    );
    ensure!(
        events
            .iter()
            .any(|event| event.contains("manifest template render failed")
                && event.contains("error_category=")),
        "expected a bounded render-failure event in {events:?}"
    );
    ensure!(
        !events.iter().any(|event| event.contains("s3cr3t-sentinel")
            || event.contains("undefined_sentinel_variable")),
        "template content must not reach telemetry: {events:?}"
    );
    Ok(())
}
