//! Bounded-telemetry assertions for the macro-invocation boundary.
//!
//! Sibling to `macros_telemetry`, which covers `render_template`. These tests
//! pin the compiled-expression fallback built by `make_macro_fn`: without them
//! the span and metrics could be deleted and the suite would still pass.
//!
//! A local `DebuggingRecorder` captures the counter and histogram without
//! touching the global recorder, and the tracing capture asserts the failure
//! event carries only the bounded error category — never the macro name or its
//! arguments.

use super::super::jinja_macros::render_template;
use crate::ast::MacroDefinition;
use crate::manifest::jinja_macros::register_macro;
use crate::test_tracing_capture::with_test_subscriber;
use anyhow::{Result as AnyResult, ensure};
use metrics_util::MetricKind;
use metrics_util::debugging::{DebugValue, DebuggingRecorder};
use minijinja::{Environment, UndefinedBehavior};
use rstest::{fixture, rstest};
use tracing_subscriber::filter::LevelFilter;

const INVOCATIONS_TOTAL: &str = "netsuke_manifest_macro_invocations_total";
const INVOCATION_DURATION: &str = "netsuke_manifest_macro_invocation_duration_seconds";

/// One drained metrics snapshot; the debugging snapshotter empties
/// histogram samples on read, so it is taken exactly once per test.
type Snapshot = Vec<(
    metrics_util::CompositeKey,
    Option<metrics::Unit>,
    Option<metrics::SharedString>,
    DebugValue,
)>;

#[fixture]
fn macro_env() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env
}

/// Run `invoke` under a local recorder and return its result alongside
/// the single drained snapshot.
fn recorded<T>(invoke: impl FnOnce() -> T) -> (T, Snapshot) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let value = metrics::with_local_recorder(&recorder, invoke);
    (value, snapshotter.snapshot().into_vec())
}

fn counter_value(snapshot: &Snapshot, outcome: &str) -> Option<u64> {
    snapshot
        .iter()
        .find_map(|(key, _unit, _description, value)| {
            if key.kind() != MetricKind::Counter || key.key().name() != INVOCATIONS_TOTAL {
                return None;
            }
            let has_outcome = key
                .key()
                .labels()
                .any(|label| label.key() == "outcome" && label.value() == outcome);
            match value {
                DebugValue::Counter(count) if has_outcome => Some(*count),
                _ => None,
            }
        })
}

fn duration_sample_count(snapshot: &Snapshot) -> usize {
    snapshot
        .iter()
        .find_map(|(key, _unit, _description, value)| {
            if key.kind() != MetricKind::Histogram || key.key().name() != INVOCATION_DURATION {
                return None;
            }
            match value {
                DebugValue::Histogram(samples) => Some(samples.len()),
                _ => None,
            }
        })
        .unwrap_or_default()
}

/// Compile `expression` against the registered macro and evaluate it, which
/// routes through the `make_macro_fn` fallback rather than a template import.
fn eval_expression(env: &Environment, expression: &str) -> Result<String, minijinja::Error> {
    env.compile_expression(expression)?
        .eval(())
        .map(|value| value.to_string())
}

#[rstest]
fn macro_invocation_records_success_telemetry(
    mut macro_env: Environment<'static>,
) -> AnyResult<()> {
    let definition = MacroDefinition {
        signature: "greet(name)".into(),
        body: "Hello {{ name }}".into(),
    };
    register_macro(&mut macro_env, &definition, 0)?;

    let (rendered, snapshot) = recorded(|| eval_expression(&macro_env, "greet('netsuke')"));

    ensure!(rendered? == "Hello netsuke", "the macro should render");
    ensure!(
        counter_value(&snapshot, "success") == Some(1),
        "a successful macro invocation should count once"
    );
    ensure!(
        duration_sample_count(&snapshot) == 1,
        "the invocation duration should record one sample"
    );
    Ok(())
}

#[rstest]
fn failed_macro_invocation_records_error_telemetry_without_macro_details(
    mut macro_env: Environment<'static>,
) -> AnyResult<()> {
    // The body reads an argument the call site does not supply, so strict
    // undefined behaviour fails inside the macro rather than at compile time.
    let definition = MacroDefinition {
        signature: "reveal(supplied)".into(),
        body: "{{ supplied }} {{ s3cr3t_sentinel_variable }}".into(),
    };
    register_macro(&mut macro_env, &definition, 0)?;

    let (events, snapshot) = {
        let ((result, events), snapshot) = recorded(|| {
            with_test_subscriber(LevelFilter::DEBUG, |captured| {
                let result = eval_expression(&macro_env, "reveal('visible')");
                (result, captured.snapshot())
            })
        });
        ensure!(
            result.is_err(),
            "a strict undefined lookup inside the macro should fail the invocation"
        );
        (events, snapshot)
    };

    ensure!(
        counter_value(&snapshot, "error") == Some(1),
        "a failed macro invocation should count once with the error outcome"
    );
    ensure!(
        duration_sample_count(&snapshot) == 1,
        "the failed invocation duration should still record one sample"
    );
    ensure!(
        events
            .iter()
            .any(|event| event.contains("manifest macro invocation failed")
                && event.contains("error_category=")),
        "expected a bounded invocation-failure event in {events:?}"
    );
    ensure!(
        !events
            .iter()
            .any(|event| event.contains("s3cr3t_sentinel_variable")
                || event.contains("reveal")
                || event.contains("visible")),
        "macro identity and arguments must not reach telemetry: {events:?}"
    );
    Ok(())
}

/// The imported-macro path is metered at the render boundary, so a template
/// render must not also emit macro-invocation metrics. This pins the boundary
/// split that keeps the two counters independently meaningful.
#[rstest]
fn imported_macro_render_does_not_emit_invocation_metrics(
    mut macro_env: Environment<'static>,
) -> AnyResult<()> {
    let definition = MacroDefinition {
        signature: "greet(name)".into(),
        body: "Hello {{ name }}".into(),
    };
    register_macro(&mut macro_env, &definition, 0)?;

    let (rendered, snapshot) =
        recorded(|| render_template(&macro_env, "{{ greet('netsuke') }}", &()));

    ensure!(
        rendered? == "Hello netsuke",
        "the imported macro should render"
    );
    ensure!(
        counter_value(&snapshot, "success").is_none(),
        "the import path renders without the compiled-expression fallback"
    );
    Ok(())
}
