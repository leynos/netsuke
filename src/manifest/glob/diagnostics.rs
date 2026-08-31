//! Bounded observability for glob expansion.
//!
//! Two outcomes of the capability-scoped walk are expected rather than
//! erroneous, so neither reaches the top-level diagnostics: a literal prefix
//! that names no directory, and a match that the capability cannot resolve
//! because a symbolic link escapes the prefix. Both are recorded here so a
//! degraded expansion is visible without having to reproduce it. The module
//! also records the manifest-scoped injected-base cache so canonicalization
//! cost and cache outcomes remain visible.
//! The Jinja adapter also records paths rejected at its shell-safety boundary.
//!
//! What is recorded is deliberately bounded and redacted.
//!
//! Metric labels carry only a closed set of outcome and reason strings, never
//! the pattern or a path, in line with the low-cardinality rule in `AGENTS.md`.
//!
//! Tracing events replace every caller-controlled path with the stable
//! `<redacted>` marker. Errors still retain the caller's pattern so they can
//! explain invalid input precisely; tracing does not need that detail to
//! identify the expansion outcome.
//!
//! What tracing does not carry is a matched path. A skipped entry is recorded
//! with the same redaction, so its relative form cannot disclose a filename
//! selected by the pattern.

use super::{GlobExpansion, GlobOutcome, GlobSkippedEntries};
use metrics::{counter, describe_counter, describe_histogram, histogram};
use std::{sync::Once, time::Duration};

/// Metric name counting glob expansions by outcome.
const EXPANSIONS_TOTAL: &str = "netsuke_manifest_glob_expansions_total";
/// Metric name counting entries dropped from a glob expansion.
const ENTRIES_SKIPPED_TOTAL: &str = "netsuke_manifest_glob_entries_skipped_total";
/// Metric name counting paths rejected by the Jinja glob adapter.
const REJECTIONS_TOTAL: &str = "netsuke_manifest_glob_rejections_total";
/// Metric name counting injected-base cache outcomes.
const BASE_CACHE_TOTAL: &str = "netsuke_manifest_glob_base_cache_total";
/// Metric name recording injected-base canonicalization latency.
const BASE_CANONICALIZATION_DURATION: &str =
    "netsuke_manifest_glob_base_canonicalization_duration_seconds";
/// Stable marker replacing caller-controlled paths in tracing events.
const REDACTED_PATH: &str = "<redacted>";

/// Register the metric descriptions once per process.
fn describe_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            EXPANSIONS_TOTAL,
            "Counts glob expansions labelled by outcome: matched, or \
             unopenable_prefix when the pattern's literal directory prefix \
             names no directory."
        );
        describe_counter!(
            ENTRIES_SKIPPED_TOTAL,
            "Counts matched entries dropped from a glob expansion, labelled \
             by reason: unreachable_symlink for a link the capability cannot \
             resolve, not_a_file for a directory or other non-file."
        );
        describe_counter!(
            REJECTIONS_TOTAL,
            "Counts paths rejected by the Jinja glob adapter, labelled by a \
             bounded outcome and error category."
        );
        describe_counter!(
            BASE_CACHE_TOTAL,
            "Counts injected manifest glob-base cache outcomes labelled by \
             outcome: bypass, hit, miss, or error."
        );
        describe_histogram!(
            BASE_CANONICALIZATION_DURATION,
            "Records the duration in seconds of injected manifest glob-base \
             canonicalization."
        );
    });
}

/// Record a relative glob that has no injected manifest base to prepare.
pub(super) fn record_base_cache_bypass() {
    describe_metrics();
    counter!(BASE_CACHE_TOTAL, "outcome" => "bypass").increment(1);
    tracing::debug!(
        operation = "glob_base_cache",
        outcome = "bypass",
        "manifest glob base preparation bypassed"
    );
}

/// Record a relative glob that reuses a canonicalized manifest base.
pub(super) fn record_base_cache_hit() {
    describe_metrics();
    counter!(BASE_CACHE_TOTAL, "outcome" => "hit").increment(1);
    tracing::debug!(
        operation = "glob_base_cache",
        outcome = "hit",
        "manifest glob base cache hit"
    );
}

/// Record a successful first canonicalization of the manifest base.
pub(super) fn record_base_cache_miss(duration: Duration) {
    record_base_cache_canonicalization("miss", duration);
    tracing::debug!(
        operation = "glob_base_cache",
        outcome = "miss",
        "manifest glob base canonicalized and cached"
    );
}

/// Record a failed canonicalization of the manifest base.
pub(super) fn record_base_cache_error(duration: Duration) {
    record_base_cache_canonicalization("error", duration);
    tracing::debug!(
        operation = "glob_base_cache",
        outcome = "error",
        error_category = "base_resolution",
        "manifest glob base preparation failed"
    );
}

/// Record the metric-only observations for one base canonicalization.
fn record_base_cache_canonicalization(outcome: &'static str, duration: Duration) {
    describe_metrics();
    counter!(BASE_CACHE_TOTAL, "outcome" => outcome).increment(1);
    record_base_canonicalization_duration(duration);
}

/// Record the elapsed duration of one injected-base canonicalization.
fn record_base_canonicalization_duration(duration: Duration) {
    histogram!(BASE_CANONICALIZATION_DURATION).record(duration.as_secs_f64());
}

/// Record a path rejected by the manifest-template shell-safety adapter.
pub(super) fn record_template_path_rejection() {
    describe_metrics();
    counter!(
        REJECTIONS_TOTAL,
        "outcome" => "unsafe_path",
        "error_category" => "shell_quoting_required"
    )
    .increment(1);
    tracing::debug!(
        path = REDACTED_PATH,
        outcome = "unsafe_path",
        error_category = "shell_quoting_required",
        "glob template path rejected"
    );
}
/// Record the observations returned by the pure glob expansion query.
pub(super) fn record(expansion: &GlobExpansion) {
    match &expansion.outcome {
        GlobOutcome::Matched => record_expansion_matched(expansion),
        GlobOutcome::UnopenablePrefix => record_unopenable_prefix(),
    }
    record_skipped_entries(&expansion.skipped);
}

/// Record an expansion that stopped because the literal prefix is unusable.
fn record_unopenable_prefix() {
    describe_metrics();
    counter!(EXPANSIONS_TOTAL, "outcome" => "unopenable_prefix").increment(1);
    tracing::debug!(
        pattern = REDACTED_PATH,
        prefix = REDACTED_PATH,
        "glob literal prefix names no directory; expanding to no matches"
    );
}

/// Record an expansion that ran the walk to completion.
fn record_expansion_matched(expansion: &GlobExpansion) {
    describe_metrics();
    counter!(EXPANSIONS_TOTAL, "outcome" => "matched").increment(1);
    tracing::debug!(
        pattern = REDACTED_PATH,
        matches = expansion.paths.len(),
        "glob expansion complete"
    );
}

/// Record a match dropped because the capability cannot resolve it.
///
/// `relative` is the match relative to the literal prefix, so it stays within
/// the scope the pattern already named.
fn record_skipped_entries(skipped: &GlobSkippedEntries) {
    describe_metrics();
    if skipped.unreachable_symlinks != 0 {
        counter!(ENTRIES_SKIPPED_TOTAL, "reason" => "unreachable_symlink")
            .increment(u64::try_from(skipped.unreachable_symlinks).unwrap_or(u64::MAX));
        for _ in &skipped.unreachable_symlink_samples {
            record_unreachable_symlink();
        }
    }
    if skipped.not_a_file != 0 {
        counter!(ENTRIES_SKIPPED_TOTAL, "reason" => "not_a_file")
            .increment(u64::try_from(skipped.not_a_file).unwrap_or(u64::MAX));
    }
}

/// Trace an unreachable symbolic-link entry retained in the bounded sample.
fn record_unreachable_symlink() {
    tracing::debug!(
        relative = REDACTED_PATH,
        "glob match traverses a symbolic link the capability cannot resolve; skipping"
    );
}
