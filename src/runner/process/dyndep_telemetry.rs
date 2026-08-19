//! Bounded observability for generated dyndep sidecar publication.
//!
//! Filesystem authority remains with the caller-provided directory capability.
//! This module records only operation outcomes and counts; it never records
//! sidecar paths or content, which may contain manifest-controlled data.

use anyhow::Result;
use metrics::{counter, describe_counter, describe_histogram, histogram};
use std::{sync::Once, time::Instant};
use tracing::field;

/// Metric name counting dyndep materialization outcomes.
pub(super) const MATERIALIZATIONS_TOTAL: &str = "netsuke_runner_dyndep_materializations_total";
/// Metric name measuring dyndep materialization duration in seconds.
pub(super) const MATERIALIZATION_DURATION: &str =
    "netsuke_runner_dyndep_materialization_duration_seconds";
/// Metric name counting bounded dyndep retention outcomes.
pub(super) const RETENTIONS_TOTAL: &str = "netsuke_runner_dyndep_retentions_total";
/// Metric name measuring bounded dyndep retention duration in seconds.
pub(super) const RETENTION_DURATION: &str = "netsuke_runner_dyndep_retention_duration_seconds";
/// Metric name counting sidecar files reclaimed by retention.
pub(super) const RETAINED_FILES_RECLAIMED: &str =
    "netsuke_runner_dyndep_retained_files_reclaimed_total";
/// Metric name counting sidecar bytes reclaimed by retention.
pub(super) const RETAINED_BYTES_RECLAIMED: &str =
    "netsuke_runner_dyndep_retained_bytes_reclaimed_total";
/// Metric name counting dyndep temporary-file collisions.
pub(super) const TEMP_FILE_RETRIES_TOTAL: &str = "netsuke_runner_dyndep_temp_file_retries_total";

/// Record a complete dyndep materialization command.
pub(super) fn instrument_materialization<T>(
    sidecar_count: usize,
    materialize: impl FnOnce() -> Result<T>,
) -> Result<T> {
    describe_metrics();
    let span = tracing::trace_span!(
        "runner.dyndep.materialize",
        sidecar_count,
        outcome = field::Empty,
        error_category = field::Empty,
    );
    let _guard = span.enter();
    let started = Instant::now();
    let result = materialize();
    let outcome = record_outcome(&span, &result, "dyndep materialization failed");
    counter!(MATERIALIZATIONS_TOTAL, "outcome" => outcome).increment(1);
    histogram!(MATERIALIZATION_DURATION).record(started.elapsed());
    result
}

/// Record an individual sidecar publication without exposing its path.
pub(super) fn instrument_sidecar_materialization<T>(
    materialize: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let span = tracing::trace_span!(
        "runner.dyndep.materialize_sidecar",
        outcome = field::Empty,
        error_category = field::Empty,
    );
    let _guard = span.enter();
    let result = materialize();
    record_outcome(&span, &result, "dyndep sidecar materialization failed");
    result
}

/// Record one bounded retention command without exposing sidecar identities.
pub(super) fn instrument_retention<T>(
    retain: impl FnOnce() -> Result<T>,
    reclaimed: impl FnOnce(&T) -> (u64, u64),
) -> Result<T> {
    describe_metrics();
    let span = tracing::trace_span!(
        "runner.dyndep.retain",
        outcome = field::Empty,
        error_category = field::Empty,
    );
    let _guard = span.enter();
    let started = Instant::now();
    let result = retain();
    let outcome = record_outcome(&span, &result, "dyndep retention failed");
    counter!(RETENTIONS_TOTAL, "outcome" => outcome).increment(1);
    histogram!(RETENTION_DURATION).record(started.elapsed());
    if let Ok(summary) = &result {
        let (files, bytes) = reclaimed(summary);
        counter!(RETAINED_FILES_RECLAIMED).increment(files);
        counter!(RETAINED_BYTES_RECLAIMED).increment(bytes);
    }
    result
}

/// Record a bounded temporary-name retry outcome without identifying a sidecar.
pub(super) fn record_temp_file_retry(outcome: &'static str) {
    describe_metrics();
    counter!(TEMP_FILE_RETRIES_TOTAL, "outcome" => outcome).increment(1);
}

/// Record a result with only bounded outcome and category fields.
fn record_outcome<T>(
    span: &tracing::Span,
    result: &Result<T>,
    failure_message: &'static str,
) -> &'static str {
    if result.is_err() {
        span.record("outcome", "error");
        span.record("error_category", "dyndep_io");
        tracing::debug!(error_category = "dyndep_io", "{failure_message}");
        "error"
    } else {
        span.record("outcome", "success");
        "success"
    }
}

/// Register metric descriptions once, before repeated materialization work.
fn describe_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            MATERIALIZATIONS_TOTAL,
            "Counts dyndep materialization outcomes by bounded outcome."
        );
        describe_histogram!(
            MATERIALIZATION_DURATION,
            "Measures dyndep materialization duration in seconds."
        );
        describe_counter!(
            RETENTIONS_TOTAL,
            "Counts bounded dyndep retention outcomes by fixed outcome."
        );
        describe_histogram!(
            RETENTION_DURATION,
            "Measures bounded dyndep retention duration in seconds."
        );
        describe_counter!(
            RETAINED_FILES_RECLAIMED,
            "Counts dyndep sidecar files reclaimed by retention."
        );
        describe_counter!(
            RETAINED_BYTES_RECLAIMED,
            "Counts dyndep sidecar bytes reclaimed by retention."
        );
        describe_counter!(
            TEMP_FILE_RETRIES_TOTAL,
            "Counts dyndep temporary-file collisions by fixed retry outcome."
        );
    });
}
