//! Bounded observability for staged dyndep bundle generation.
//!
//! The generator remains an effect-free query. This module owns its spans and
//! metrics so rendering code stays concerned only with constructing Ninja
//! artefacts. Every field is either a count or a fixed outcome label; graph
//! paths, action identifiers, and rendered content are deliberately excluded.

use crate::ninja_gen::NinjaGenError;
use metrics::{counter, describe_counter, describe_histogram, histogram};
use std::{sync::Once, time::Instant};
use tracing::field;

const BUNDLE_GENERATIONS_TOTAL: &str = "netsuke_ninja_dyndep_bundle_generations_total";
const BUNDLE_GENERATION_DURATION: &str = "netsuke_ninja_dyndep_bundle_generation_duration_seconds";

/// Record one complete bundle-generation query.
pub(super) fn instrument_bundle_generation<T>(
    action_count: usize,
    target_count: usize,
    generate: impl FnOnce() -> Result<T, NinjaGenError>,
) -> Result<T, NinjaGenError> {
    describe_metrics();
    let span = tracing::trace_span!(
        "ninja.dyndep.bundle.generate",
        action_count,
        target_count,
        outcome = field::Empty,
        error_category = field::Empty,
    );
    let _guard = span.enter();
    let started = Instant::now();
    let result = generate();
    let outcome = record_outcome(&span, &result, "bundle generation failed");
    counter!(BUNDLE_GENERATIONS_TOTAL, "outcome" => outcome).increment(1);
    histogram!(BUNDLE_GENERATION_DURATION).record(started.elapsed());
    result
}

/// Record one serial-edge lowering attempt without exposing graph identity.
pub(super) fn instrument_serial_lowering<T>(
    dependency_count: usize,
    render: impl FnOnce() -> Result<T, NinjaGenError>,
) -> Result<T, NinjaGenError> {
    let span = tracing::trace_span!(
        "ninja.dyndep.serial.lower",
        dependency_count,
        outcome = field::Empty,
        error_category = field::Empty,
    );
    let _guard = span.enter();
    let result = render();
    record_outcome(&span, &result, "serial dependency lowering failed");
    result
}

/// Record the bounded result of one generation operation.
fn record_outcome<T>(
    span: &tracing::Span,
    result: &Result<T, NinjaGenError>,
    failure_message: &'static str,
) -> &'static str {
    if result.is_err() {
        span.record("outcome", "error");
        span.record("error_category", "ninja_generation");
        tracing::debug!(error_category = "ninja_generation", "{failure_message}");
        "error"
    } else {
        span.record("outcome", "success");
        "success"
    }
}

/// Register metric descriptions once, outside the rendering query itself.
fn describe_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            BUNDLE_GENERATIONS_TOTAL,
            "Counts dyndep bundle generation outcomes by bounded outcome."
        );
        describe_histogram!(
            BUNDLE_GENERATION_DURATION,
            "Measures dyndep bundle generation duration in seconds."
        );
    });
}
