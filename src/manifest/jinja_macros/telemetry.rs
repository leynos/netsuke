//! Bounded observability for manifest template rendering and macro invocation.
//!
//! Rendering a template and invoking a macro are queries: they compute a value
//! and are expected to be free of ambient concerns. Spans, timing, and metric
//! emission therefore live here rather than interleaved with the evaluation
//! logic, so `render_template` and the Jinja callback read as plain evaluation
//! and each instrumented boundary composes explicitly.
//!
//! Collecting the telemetry in one module also gives its privacy contract a
//! single place to be reviewed: every field emitted here is bounded by
//! construction, so manifest-controlled data — template text, macro names,
//! context values, environment variable names — cannot reach a subscriber.

use crate::manifest::budget::ManifestBudgetStage;
use metrics::{counter, describe_counter, describe_histogram, histogram};
use minijinja::Error;
use std::{sync::Once, time::Instant};
use tracing::field;

/// Metric name counting macro-invocation outcomes.
const MACRO_INVOCATIONS_TOTAL: &str = "netsuke_manifest_macro_invocations_total";
/// Metric name for macro-invocation duration histograms.
const MACRO_INVOCATION_DURATION: &str = "netsuke_manifest_macro_invocation_duration_seconds";
/// Metric name counting template-render outcomes.
const TEMPLATE_RENDERS_TOTAL: &str = "netsuke_manifest_template_renders_total";
/// Metric name for template-render duration histograms.
const TEMPLATE_RENDER_DURATION: &str = "netsuke_manifest_template_render_duration_seconds";
/// Metric name counting deterministic manifest-budget exhaustion events.
const BUDGET_EXHAUSTED_TOTAL: &str = "netsuke_manifest_budget_exhausted_total";

/// Register the macro-invocation metric descriptions exactly once.
///
/// Called when a macro is registered rather than per invocation, so the
/// one-time guard never sits on the rendering hot path.
pub(super) fn describe_macro_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            MACRO_INVOCATIONS_TOTAL,
            "Counts manifest macro invocation outcomes labelled as success or error."
        );
        describe_histogram!(
            MACRO_INVOCATION_DURATION,
            "Measures manifest macro invocation duration in seconds."
        );
    });
}

/// Register the template-render metric descriptions exactly once.
///
/// Called from the instrumentation wrapper rather than from `render_template`,
/// so rendering never reaches for the metric registry itself. The macro
/// counterpart is registered when a macro is registered, which is setup rather
/// than evaluation, so it has a natural home outside the query.
fn describe_render_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            TEMPLATE_RENDERS_TOTAL,
            "Counts manifest template renders by bounded outcome and macro-import presence."
        );
        describe_histogram!(
            TEMPLATE_RENDER_DURATION,
            "Measures manifest template rendering duration in seconds."
        );
        describe_counter!(
            BUDGET_EXHAUSTED_TOTAL,
            "Counts manifest resource budget exhaustion by closed-vocabulary stage and budget."
        );
    });
}

/// Record one resource budget exhaustion without manifest-controlled labels.
pub(crate) fn record_budget_exhaustion(stage: &'static str, budget: &'static str) {
    describe_render_metrics();
    debug_assert!(
        ManifestBudgetStage::all()
            .iter()
            .any(|known_stage| known_stage.as_str() == stage),
        "budget telemetry must use a closed stage vocabulary"
    );
    counter!(BUDGET_EXHAUSTED_TOTAL, "stage" => stage, "budget" => budget).increment(1);
}

/// Evaluate `invoke` inside a macro-invocation span, recording its outcome.
///
/// The span and metrics carry only the outcome and, on failure, the `MiniJinja`
/// error kind; the macro's identity and arguments stay out of telemetry.
pub(super) fn instrument_macro_invocation<T>(
    invoke: impl FnOnce() -> Result<T, Error>,
) -> Result<T, Error> {
    let span = tracing::trace_span!(
        "manifest.macro.invoke",
        outcome = field::Empty,
        error_category = field::Empty,
    );
    let _guard = span.enter();
    let started = Instant::now();
    let result = invoke();
    let outcome = outcome_label(&result);
    span.record("outcome", outcome);
    if let Err(error) = &result {
        span.record("error_category", format_args!("{:?}", error.kind()));
        tracing::debug!(error_category = ?error.kind(), "manifest macro invocation failed");
        if error.kind() == minijinja::ErrorKind::OutOfFuel {
            record_budget_exhaustion("macro", "fuel");
        }
    }
    counter!(MACRO_INVOCATIONS_TOTAL, "outcome" => outcome).increment(1);
    histogram!(MACRO_INVOCATION_DURATION).record(started.elapsed());
    result
}

/// Evaluate `render` inside a template-render span, recording its outcome.
///
/// `has_macro_imports` is a bounded shape signal, not content: it distinguishes
/// the import-prefixed render path from the plain one without revealing which
/// macros a manifest defines.
pub(super) fn instrument_template_render<T>(
    has_macro_imports: bool,
    render: impl FnOnce() -> Result<T, Error>,
) -> Result<T, Error> {
    describe_render_metrics();
    let span = tracing::trace_span!(
        "manifest.template.render",
        outcome = field::Empty,
        has_macro_imports,
        error_category = field::Empty,
    );
    let _guard = span.enter();
    let started = Instant::now();
    let result = render();
    let outcome = outcome_label(&result);
    span.record("outcome", outcome);
    if let Err(error) = &result {
        span.record("error_category", format_args!("{:?}", error.kind()));
        tracing::debug!(error_category = ?error.kind(), "manifest template render failed");
    }
    counter!(
        TEMPLATE_RENDERS_TOTAL,
        "outcome" => outcome,
        "has_macro_imports" => if has_macro_imports { "true" } else { "false" },
    )
    .increment(1);
    histogram!(TEMPLATE_RENDER_DURATION).record(started.elapsed());
    result
}

/// Reduce a result to a low-cardinality outcome label.
const fn outcome_label<T>(result: &Result<T, Error>) -> &'static str {
    if result.is_ok() { "success" } else { "error" }
}
